use async_channel::Sender;
use ksni::menu::{CheckmarkItem, StandardItem};

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    Show,
    Projects,
    Manage,
    Settings,
    ToggleAutostart,
    Quit,
}

pub struct Tray {
    sender: Sender<TrayCommand>,
    pub autostart: bool,
    pub shortcut: String,
}

impl Tray {
    pub fn new(sender: Sender<TrayCommand>, autostart: bool, shortcut: String) -> Self {
        Self {
            sender,
            autostart,
            shortcut,
        }
    }

    fn send(&self, command: TrayCommand) {
        let _ = self.sender.try_send(command);
    }
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "radial-deck".into()
    }

    fn title(&self) -> String {
        "RadialDeck".into()
    }

    fn icon_name(&self) -> String {
        "io.github.raihan.RadialDeck".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Show);
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "RadialDeck".into(),
            description: format!("Quick actions: {}", self.shortcut),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::MenuItem;
        vec![
            StandardItem {
                label: "Quick Actions".into(),
                shortcut: vec![shortcut_keys(&self.shortcut)],
                activate: Box::new(|tray: &mut Tray| tray.send(TrayCommand::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Project Folders…".into(),
                activate: Box::new(|tray: &mut Tray| tray.send(TrayCommand::Projects)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Manage Folders…".into(),
                activate: Box::new(|tray: &mut Tray| tray.send(TrayCommand::Manage)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings…".into(),
                activate: Box::new(|tray: &mut Tray| tray.send(TrayCommand::Settings)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Launch at Login".into(),
                checked: self.autostart,
                activate: Box::new(|tray: &mut Tray| tray.send(TrayCommand::ToggleAutostart)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit RadialDeck".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Tray| tray.send(TrayCommand::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Converts `CTRL+ALT+SPACE` to the ksni shortcut representation.
fn shortcut_keys(shortcut: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for token in shortcut.split('+') {
        let token = token.trim();
        let display = match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Control".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "super" | "meta" | "win" => "Super".to_string(),
            "" => continue,
            "space" => "Space".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => continue,
                }
            }
        };
        keys.push(display);
    }
    keys
}
