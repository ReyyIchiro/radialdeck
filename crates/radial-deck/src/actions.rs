use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gtk::gio;
use gtk::glib::value::ToValue;
use gtk::prelude::*;

const TERMINALS: [&str; 8] = [
    "konsole",
    "kitty",
    "alacritty",
    "foot",
    "wezterm",
    "gnome-terminal",
    "xfce4-terminal",
    "kgx",
];

/// Opens a folder with the desktop's default file manager. Arguments are
/// passed directly; no shell is involved.
pub fn open_folder(path: &Path) -> io::Result<()> {
    spawn_detached(Command::new("xdg-open").arg(path))
}

pub fn copy_path_to_clipboard(path: &Path) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(&path.to_string_lossy());
    }
}

pub fn copy_paths_to_clipboard(paths: &[PathBuf]) {
    if let Some(display) = gtk::gdk::Display::default() {
        let text = paths
            .iter()
            .map(|path| path.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        display.clipboard().set_text(&text);
    }
}

/// Puts real file references on the desktop clipboard. Dolphin, Nautilus and
/// other file managers receive `text/uri-list` through GTK's content provider.
pub fn copy_files_to_clipboard(paths: &[PathBuf]) -> Result<(), String> {
    let display = gtk::gdk::Display::default()
        .ok_or_else(|| "No desktop clipboard is available".to_owned())?;
    let files = paths.iter().map(gio::File::for_path).collect::<Vec<_>>();
    let list = gtk::gdk::FileList::from_array(&files);
    let value = list.to_value();
    let provider = gtk::gdk::ContentProvider::for_value(&value);
    display
        .clipboard()
        .set_content(Some(&provider))
        .map_err(|error| format!("Could not copy files: {error}"))
}

#[derive(Debug, Clone)]
pub struct KdeDevice {
    pub id: String,
    pub name: String,
}

/// Reads reachable, paired KDE Connect devices. This is the Linux counterpart
/// of AirDrop; it is invoked only when the actions wheel opens.
pub fn kdeconnect_devices() -> Vec<KdeDevice> {
    let Some(binary) = find_in_path("kdeconnect-cli") else {
        return Vec::new();
    };
    let Ok(output) = Command::new(binary)
        .args(["--list-available", "--id-name-only"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (id, name) = line.trim().split_once(' ')?;
            if id.is_empty() || name.trim().is_empty() {
                return None;
            }
            Some(KdeDevice {
                id: id.to_owned(),
                name: name.trim().to_owned(),
            })
        })
        .collect()
}

pub fn share_via_kdeconnect(paths: &[PathBuf], device: &str) -> Result<(), String> {
    if !is_valid_executable_name(device) && !device.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("Invalid device id".into());
    }
    let binary =
        find_in_path("kdeconnect-cli").ok_or_else(|| "KDE Connect is not installed".to_owned())?;
    for path in paths {
        let output = Command::new(&binary)
            .arg("--share")
            .arg(path)
            .arg("-d")
            .arg(device)
            .output()
            .map_err(|error| format!("KDE Connect: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
    }
    Ok(())
}

pub fn available_terminals() -> Vec<&'static str> {
    TERMINALS
        .into_iter()
        .filter(|terminal| find_in_path(terminal).is_some())
        .collect()
}

/// Opens a terminal in `path`. The preference is an executable name only;
/// flags are chosen from a fixed table, so user input cannot inject arguments.
pub fn open_terminal(path: &Path, preferred: Option<&str>) -> io::Result<()> {
    let terminal = match preferred {
        Some(name) => {
            if !is_valid_executable_name(name) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "terminal must be a plain executable name",
                ));
            }
            name.to_owned()
        }
        None => available_terminals()
            .first()
            .map(|name| (*name).to_owned())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no terminal found"))?,
    };

    let mut command = Command::new(&terminal);
    match terminal.as_str() {
        "konsole" => command.arg("--workdir").arg(path),
        "kitty" => command.arg("--directory").arg(path),
        "alacritty" => command.arg("--working-directory").arg(path),
        "foot" => command.arg("--working-directory").arg(path),
        "wezterm" => command.arg("start").arg("--cwd").arg(path),
        "gnome-terminal" | "kgx" => command.arg("--working-directory").arg(path),
        "xfce4-terminal" => command.arg("--working-directory").arg(path),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("unsupported terminal: {terminal}"),
            ));
        }
    };
    spawn_detached(&mut command)
}

fn spawn_detached(command: &mut Command) -> io::Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

fn is_valid_executable_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(|ch| ch.is_whitespace() || ch.is_control())
}

pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.is_file()
            && std::fs::metadata(path)
                .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_name_validation_rejects_paths_and_flags() {
        assert!(is_valid_executable_name("konsole"));
        assert!(!is_valid_executable_name("/usr/bin/konsole"));
        assert!(!is_valid_executable_name("konsole --hold"));
        assert!(!is_valid_executable_name(""));
        assert!(!is_valid_executable_name("konsole\nrm"));
    }

    #[test]
    fn path_lookup_does_not_accept_directories() {
        assert!(find_in_path("definitely-not-a-real-terminal-xyz").is_none());
    }
}
