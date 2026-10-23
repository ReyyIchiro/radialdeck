use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::CreateSessionOptions;
use futures_util::StreamExt;

use crate::APP_ID;

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Activated,
    /// The portal's human-readable description of the active binding.
    Bound(String),
    Error(String),
}

/// Owns the portal session. Dropping it stops the worker thread's channels;
/// the D-Bus session itself is closed by the portal when the process exits.
pub struct HotkeyService {
    configure_tx: async_channel::Sender<()>,
}

pub fn start(preferred: String, events: async_channel::Sender<HotkeyEvent>) -> HotkeyService {
    let (configure_tx, configure_rx) = async_channel::unbounded();

    let thread = std::thread::Builder::new()
        .name("hotkey-portal".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = events.send_blocking(HotkeyEvent::Error(format!(
                        "cannot start async runtime: {error}"
                    )));
                    return;
                }
            };
            runtime.block_on(run(preferred, events, configure_rx));
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
    preferred: String,
    events: async_channel::Sender<HotkeyEvent>,
    configure_rx: async_channel::Receiver<()>,
) {
    let connection = match ashpd::zbus::Connection::session().await {
        Ok(connection) => connection,
        Err(error) => {
            let _ = events
                .send(HotkeyEvent::Error(format!(
                    "cannot connect to the session bus: {error}"
                )))
                .await;
            return;
        }
    };

    if let Err(error) = register_host_app(&connection).await {
        tracing::debug!("host portal registration skipped: {error}");
    }

    let portal = match GlobalShortcuts::with_connection(connection).await {
        Ok(portal) => portal,
        Err(error) => {
            let _ = events
                .send(HotkeyEvent::Error(format!(
                    "GlobalShortcuts portal unavailable: {error}"
                )))
                .await;
            return;
        }
    };

    let session = match portal.create_session(CreateSessionOptions::default()).await {
        Ok(session) => session,
        Err(error) => {
            let _ = events
                .send(HotkeyEvent::Error(format!(
                    "cannot create shortcut session: {error}"
                )))
                .await;
            return;
        }
    };

    let trigger = if preferred.trim().is_empty() {
        None
    } else {
        Some(preferred.as_str())
    };
    let shortcut =
        NewShortcut::new("toggle", "Show RadialDeck launcher").preferred_trigger(trigger);
    let request = match portal
        .bind_shortcuts(&session, &[shortcut], None, BindShortcutsOptions::default())
        .await
    {
        Ok(request) => request,
        Err(error) => {
            let _ = events
                .send(HotkeyEvent::Error(format!("cannot bind shortcut: {error}")))
                .await;
            return;
        }
    };

    match request.response() {
        Ok(bound) => {
            let description = bound
                .shortcuts()
                .first()
                .map(|shortcut| shortcut.trigger_description().to_string())
                .unwrap_or_default();
            if description.is_empty() {
                // KDE keeps previously denied bindings around: BindShortcuts
                // succeeds but without a trigger, so nothing is grabbed.
                let _ = events
                    .send(HotkeyEvent::Error(format!(
                        "no shortcut assigned; use Change Shortcut… to bind {}",
                        preferred
                    )))
                    .await;
            } else {
                let _ = events.send(HotkeyEvent::Bound(description)).await;
            }
        }
        Err(error) => {
            let _ = events
                .send(HotkeyEvent::Error(format!(
                    "shortcut was not registered: {error}"
                )))
                .await;
            return;
        }
    }

    let mut activated = match portal.receive_activated().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = events
                .send(HotkeyEvent::Error(format!(
                    "cannot listen for shortcut: {error}"
                )))
                .await;
            return;
        }
    };
    let mut changed = portal.receive_shortcuts_changed().await.ok();

    loop {
        tokio::select! {
            event = activated.next() => {
                match event {
                    Some(event) if event.shortcut_id() == "toggle" => {
                        let _ = events.send(HotkeyEvent::Activated).await;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            Ok(()) = configure_rx.recv() => {
                if let Err(error) = portal
                    .configure_shortcuts(&session, None, ConfigureShortcutsOptions::default())
                    .await
                {
                    let _ = events
                        .send(HotkeyEvent::Error(format!(
                            "cannot open shortcut settings: {error}"
                        )))
                        .await;
                    continue;
                }
                if let Ok(updated) = refresh_description(&portal, &session).await {
                    let _ = events.send(HotkeyEvent::Bound(updated)).await;
                }
            }
            Some(update) = async {
                match changed.as_mut() {
                    Some(stream) => stream.next().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(shortcut) = update.shortcuts().first() {
                    let _ = events
                        .send(HotkeyEvent::Bound(shortcut.trigger_description().to_string()))
                        .await;
                }
            }
            else => break,
        }
    }
}

async fn refresh_description(
    portal: &GlobalShortcuts,
    session: &ashpd::desktop::Session<GlobalShortcuts>,
) -> Result<String, ashpd::Error> {
    let request = portal.list_shortcuts(session, Default::default()).await?;
    let list = request.response()?;
    Ok(list
        .shortcuts()
        .first()
        .map(|shortcut| shortcut.trigger_description().to_string())
        .unwrap_or_default())
}
