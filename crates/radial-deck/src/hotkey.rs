use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::CreateSessionOptions;
use ashpd::zbus::fdo;
use futures_util::stream::BoxStream;
use futures_util::StreamExt;

use crate::APP_ID;

pub const QUICK_ID: &str = "toggle";
pub const PROJECTS_ID: &str = "projects";

/// Well-known name owned by the desktop portal daemon.
const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";

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

/// First delay between registration attempts. Doubles until [`RETRY_MAX`].
const RETRY_INITIAL: std::time::Duration = std::time::Duration::from_secs(2);
/// Upper bound for the retry delay so a broken portal is probed at most
/// once a minute without hammering the bus.
const RETRY_MAX: std::time::Duration = std::time::Duration::from_secs(60);

/// Keeps the shortcut registration alive for the lifetime of the process.
///
/// The portal is frequently unavailable right after login: `xdg-desktop-portal`
/// may start before the desktop exports `XDG_CURRENT_DESKTOP`, in which case it
/// picks a backend without `GlobalShortcuts`, or it may be restarted while the
/// app runs. Giving up on the first error would leave the hotkeys dead until
/// the user restarts the app by hand, so retry with exponential backoff.
async fn run(
    bindings: Bindings,
    events: async_channel::Sender<HotkeyEvent>,
    configure_rx: async_channel::Receiver<()>,
) {
    let mut delay = RETRY_INITIAL;
    loop {
        match serve(&bindings, &events, &configure_rx).await {
            ServeEnd::Stopped => break,
            ServeEnd::Retry { message, bound } => {
                if events.is_closed() {
                    break;
                }
                // A session that did work and then dropped is not a sign of a
                // broken portal, so start over with the short delay.
                if bound {
                    delay = RETRY_INITIAL;
                }
                send_error(
                    &events,
                    "all",
                    format!("{message}; retrying in {}s", delay.as_secs().max(1)),
                );
                tracing::warn!(
                    "global shortcuts unavailable: {message}; retrying in {}s",
                    delay.as_secs().max(1)
                );
                tokio::time::sleep(delay).await;
                delay = next_delay(delay);
            }
        }
    }
}

/// Doubles the retry delay up to [`RETRY_MAX`]. Kept separate from the loop so
/// the growth policy can be tested without a portal.
fn next_delay(current: std::time::Duration) -> std::time::Duration {
    (current * 2).min(RETRY_MAX)
}

enum ServeEnd {
    /// Channels are closed; the app is shutting down.
    Stopped,
    /// Registration failed or the portal went away. Worth retrying.
    /// `bound` records whether shortcuts had been registered before the loss.
    Retry { message: String, bound: bool },
}

async fn serve(
    bindings: &Bindings,
    events: &async_channel::Sender<HotkeyEvent>,
    configure_rx: &async_channel::Receiver<()>,
) -> ServeEnd {
    let connection = match ashpd::zbus::Connection::session().await {
        Ok(connection) => connection,
        Err(error) => {
            return ServeEnd::Retry {
                message: format!("cannot connect to the session bus: {error}"),
                bound: false,
            };
        }
    };

    if let Err(error) = register_host_app(&connection).await {
        tracing::debug!("host portal registration skipped: {error}");
    }

    // Keep a handle for the bus-name watch: the shortcut session consumes the
    // connection, and `Connection` is a cheap handle over a shared socket.
    let bus = connection.clone();
    let portal = match GlobalShortcuts::with_connection(connection).await {
        Ok(portal) => portal,
        Err(error) => {
            return ServeEnd::Retry {
                message: format!(
                    "the desktop portal does not offer global shortcuts ({error}). \
                     On KDE this happens when xdg-desktop-portal started before \
                     XDG_CURRENT_DESKTOP was set, so it fell back to the GTK \
                     backend. Restarting the portal fixes it"
                ),
                bound: false,
            };
        }
    };

    let session = match portal.create_session(CreateSessionOptions::default()).await {
        Ok(session) => session,
        Err(error) => {
            return ServeEnd::Retry {
                message: format!("cannot create shortcut session: {error}"),
                bound: false,
            };
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
            return ServeEnd::Retry {
                message: format!("cannot bind shortcuts: {error}"),
                bound: false,
            };
        }
    };

    match request.response() {
        Ok(bound) => report_shortcuts(events, bindings, bound.shortcuts()),
        Err(error) => {
            return ServeEnd::Retry {
                message: format!("shortcuts were not registered: {error}"),
                bound: false,
            };
        }
    }

    let mut activated = match portal.receive_activated().await {
        Ok(stream) => stream,
        Err(error) => {
            return ServeEnd::Retry {
                message: format!("cannot listen for shortcuts: {error}"),
                bound: false,
            };
        }
    };
    let mut changed = portal.receive_shortcuts_changed().await.ok();
    // `receive_activated` keeps waiting on the same bus connection, so it does
    // not end when the portal daemon restarts: the connection to the bus is
    // still alive, only the well-known name moved. Watch that name instead.
    let mut portal_owner = watch_portal_owner(&bus).await;

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
                    // The portal dropped the session: it was restarted or
                    // crashed. Register again instead of going silent.
                    None => {
                        return ServeEnd::Retry {
                            message: "the portal closed the shortcut session".to_owned(),
                            bound: true,
                        };
                    }
                }
            }
            _owner = async {
                match portal_owner.as_mut() {
                    Some(stream) => stream.next().await,
                    None => std::future::pending().await,
                }
            } => {
                // Any change to the portal's owner invalidates the session: the
                // daemon that holds it is gone. The stream ending (`None`) means
                // the bus connection itself died. Both need a fresh registration.
                return ServeEnd::Retry {
                    message: "the desktop portal restarted".to_owned(),
                    bound: true,
                };
            }
            result = configure_rx.recv() => {
                if result.is_err() {
                    return ServeEnd::Stopped;
                }
                if let Err(error) = portal
                    .configure_shortcuts(&session, None, ConfigureShortcutsOptions::default())
                    .await
                {
                    send_error(
                        events,
                        "all",
                        format!("cannot open shortcut settings: {error}"),
                    );
                    continue;
                }
                match list_shortcuts(&portal, &session).await {
                    Ok(shortcuts) => report_shortcuts(events, bindings, &shortcuts),
                    Err(error) => send_error(
                        events,
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
                report_shortcuts(events, bindings, update.shortcuts());
            }
        }
    }
}

fn trigger(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// Stream of the portal's bus owner, yielding `Some(None)` when the name loses
/// its owner and `None` when the signal stream ends.
///
/// A portal restart keeps the session bus alive, so the D-Bus connection used
/// by the shortcut session stays valid and no error surfaces on it. Watching
/// the well-known name is the only reliable way to notice that the daemon
/// behind it was replaced.
type PortalOwnerStream =
    BoxStream<'static, Option<Option<ashpd::zbus::names::UniqueName<'static>>>>;

async fn watch_portal_owner(connection: &ashpd::zbus::Connection) -> Option<PortalOwnerStream> {
    let proxy = match fdo::DBusProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            tracing::debug!("cannot watch the portal bus name: {error}");
            return None;
        }
    };
    let stream = match proxy
        .receive_name_owner_changed_with_args(&[(0, PORTAL_BUS_NAME)])
        .await
    {
        Ok(stream) => stream,
        Err(error) => {
            tracing::debug!("cannot subscribe to portal name changes: {error}");
            return None;
        }
    };
    // Only an empty new owner means the portal went away; a new unique name
    // means systemd restarted it and the caller must re-register.
    Some(Box::pin(stream.map(|signal| {
        let args = signal.args().ok()?;
        Some(args.new_owner().as_ref().map(|owner| owner.to_owned()))
    })))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_doubles_then_saturates() {
        let mut delay = RETRY_INITIAL;
        let mut seen = vec![delay];
        for _ in 0..10 {
            delay = next_delay(delay);
            seen.push(delay);
        }
        assert_eq!(seen[0].as_secs(), 2);
        assert_eq!(seen[1].as_secs(), 4);
        assert_eq!(seen[2].as_secs(), 8);
        assert_eq!(seen[3].as_secs(), 16);
        assert_eq!(seen[4].as_secs(), 32);
        assert_eq!(seen[5].as_secs(), 60, "clamped to the maximum");
        assert_eq!(seen[10].as_secs(), 60, "stays clamped");
    }

    #[test]
    fn trigger_ignores_blank_bindings() {
        assert_eq!(trigger("  "), None);
        assert_eq!(trigger(""), None);
        assert_eq!(trigger(" <Super>space "), Some("<Super>space"));
    }
}
