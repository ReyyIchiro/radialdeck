use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::CreateSessionOptions;
use futures_util::StreamExt;

use crate::APP_ID;

pub const QUICK_ID: &str = "toggle";
pub const PROJECTS_ID: &str = "projects";

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Activated(String),
    /// The portal's human-readable description of the active binding.
    Bound {
        id: String,
        description: String,
    },
    Error {
        id: String,
        message: String,
    },
}

#[derive(Debug, Clone)]
struct Bindings {
    quick: String,
    projects: String,
}

impl Bindings {
    fn preferred(&self, id: &str) -> &str {
        if id == PROJECTS_ID {
            &self.projects
        } else {
            &self.quick
        }
    }

    fn label(id: &str) -> &'static str {
        if id == PROJECTS_ID {
            "project folders"
        } else {
            "quick actions"
        }
    }
}

/// Owns the portal session. Dropping it stops the worker thread's channels;
/// the D-Bus session itself is closed by the portal when the process exits.
pub struct HotkeyService {
    configure_tx: async_channel::Sender<()>,
}

pub fn start(
    quick: String,
    projects: String,
    events: async_channel::Sender<HotkeyEvent>,
) -> HotkeyService {
    let (configure_tx, configure_rx) = async_channel::unbounded();
    let bindings = Bindings { quick, projects };

    let thread = std::thread::Builder::new()
        .name("hotkey-portal".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    send_error(
                        &events,
                        "all",
                        format!("cannot start async runtime: {error}"),
                    );
                    return;
                }
            };
            runtime.block_on(run(bindings, events, configure_rx));
        });

    if let Err(error) = thread {
        eprintln!("radial-deck: cannot start hotkey thread: {error}");
    }

    HotkeyService { configure_tx }
}

impl HotkeyService {
    pub fn configure(&self) {
        let _ = self.configure_tx.send_blocking(());
    }
}

/// Associates this D-Bus connection with the desktop file so the portal
/// knows which application is requesting the shortcut. Without it, launchers
/// that do not place the process in an app-named systemd scope are rejected
/// with "An app id is required".
async fn register_host_app(connection: &ashpd::zbus::Connection) -> ashpd::zbus::Result<()> {
    use std::collections::HashMap;

    let proxy = ashpd::zbus::Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.host.portal.Registry",
    )
    .await?;
    let options: HashMap<&str, ashpd::zvariant::Value<'static>> = HashMap::new();
    proxy.call::<_, _, ()>("Register", &(APP_ID, options)).await
}

async fn run(
    bindings: Bindings,
    events: async_channel::Sender<HotkeyEvent>,
    configure_rx: async_channel::Receiver<()>,
) {
    let connection = match ashpd::zbus::Connection::session().await {
        Ok(connection) => connection,
        Err(error) => {
            send_error(
                &events,
                "all",
                format!("cannot connect to the session bus: {error}"),
            );
            return;
        }
    };

    if let Err(error) = register_host_app(&connection).await {
        tracing::debug!("host portal registration skipped: {error}");
    }

    let portal = match GlobalShortcuts::with_connection(connection).await {
        Ok(portal) => portal,
        Err(error) => {
            send_error(
                &events,
                "all",
                format!("GlobalShortcuts portal unavailable: {error}"),
            );
            return;
        }
    };

    let session = match portal.create_session(CreateSessionOptions::default()).await {
        Ok(session) => session,
        Err(error) => {
            send_error(
                &events,
                "all",
                format!("cannot create shortcut session: {error}"),
            );
            return;
        }
    };

    let shortcuts = [
        NewShortcut::new(QUICK_ID, "Quick actions (files and archives)")
            .preferred_trigger(trigger(&bindings.quick)),
        NewShortcut::new(PROJECTS_ID, "Project folders")
            .preferred_trigger(trigger(&bindings.projects)),
    ];
    let request = match portal
        .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
        .await
    {
        Ok(request) => request,
        Err(error) => {
            send_error(&events, "all", format!("cannot bind shortcuts: {error}"));
            return;
        }
    };

    match request.response() {
        Ok(bound) => report_shortcuts(&events, &bindings, bound.shortcuts()),
        Err(error) => {
            send_error(
                &events,
                "all",
                format!("shortcuts were not registered: {error}"),
            );
            return;
        }
    }

    let mut activated = match portal.receive_activated().await {
        Ok(stream) => stream,
        Err(error) => {
            send_error(
                &events,
                "all",
                format!("cannot listen for shortcuts: {error}"),
            );
            return;
        }
    };
    let mut changed = portal.receive_shortcuts_changed().await.ok();

    loop {
        tokio::select! {
            event = activated.next() => {
                match event {
                    Some(event) => {
                        let id = event.shortcut_id();
                        if id == QUICK_ID || id == PROJECTS_ID {
                            let _ = events.send(HotkeyEvent::Activated(id.to_owned())).await;
                        }
                    }
                    None => break,
                }
            }
            Ok(()) = configure_rx.recv() => {
                if let Err(error) = portal
                    .configure_shortcuts(&session, None, ConfigureShortcutsOptions::default())
                    .await
                {
                    send_error(
                        &events,
                        "all",
                        format!("cannot open shortcut settings: {error}"),
                    );
                    continue;
                }
                match list_shortcuts(&portal, &session).await {
                    Ok(shortcuts) => report_shortcuts(&events, &bindings, &shortcuts),
                    Err(error) => send_error(
                        &events,
                        "all",
                        format!("cannot refresh shortcuts: {error}"),
                    ),
                }
            }
            Some(update) = async {
                match changed.as_mut() {
                    Some(stream) => stream.next().await,
                    None => std::future::pending().await,
                }
            } => {
                report_shortcuts(&events, &bindings, update.shortcuts());
            }
            else => break,
        }
    }
}

fn trigger(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn send_error(events: &async_channel::Sender<HotkeyEvent>, id: &str, message: String) {
    let _ = events.try_send(HotkeyEvent::Error {
        id: id.to_owned(),
        message,
    });
}

fn report_shortcuts(
    events: &async_channel::Sender<HotkeyEvent>,
    bindings: &Bindings,
    shortcuts: &[ashpd::desktop::global_shortcuts::Shortcut],
) {
    for shortcut in shortcuts {
        let id = shortcut.id();
        if id != QUICK_ID && id != PROJECTS_ID {
            continue;
        }
        let description = shortcut.trigger_description().to_string();
        if description.is_empty() {
            // KDE keeps previously denied bindings around: BindShortcuts
            // succeeds but without a trigger, so nothing is grabbed.
            let _ = events.try_send(HotkeyEvent::Error {
                id: id.to_owned(),
                message: format!(
                    "no shortcut assigned to {}; use Change Shortcuts… to bind {}",
                    Bindings::label(id),
                    bindings.preferred(id)
                ),
            });
        } else {
            let _ = events.try_send(HotkeyEvent::Bound {
                id: id.to_owned(),
                description,
            });
        }
    }
}

async fn list_shortcuts(
    portal: &GlobalShortcuts,
    session: &ashpd::desktop::Session<GlobalShortcuts>,
) -> Result<Vec<ashpd::desktop::global_shortcuts::Shortcut>, ashpd::Error> {
    let request = portal.list_shortcuts(session, Default::default()).await?;
    let list = request.response()?;
    Ok(list.shortcuts().to_vec())
}
