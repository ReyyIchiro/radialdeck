use std::fs;
use std::io;
use std::path::PathBuf;

pub const DESKTOP_ID: &str = "io.github.raihan.RadialDeck";

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.config_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn autostart_path() -> PathBuf {
    config_home()
        .join("autostart")
        .join(format!("{DESKTOP_ID}.desktop"))
}

pub fn is_enabled() -> bool {
    autostart_path().is_file()
}

pub fn set_enabled(enabled: bool, executable: &std::path::Path) -> io::Result<()> {
    let path = autostart_path();
    if !enabled {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=RadialDeck\n\
         Comment=Radial folder launcher\n\
         Exec={} --hidden\n\
         Icon={DESKTOP_ID}\n\
         Terminal=false\n\
         StartupNotify=false\n\
         X-GNOME-Autostart-enabled=true\n",
        quote_exec(&executable.to_string_lossy())
    );
    fs::write(&path, contents)
}

/// Quotes an Exec path per the Desktop Entry spec (double quotes with
/// backslash escapes for `"`, `` ` ``, `$` and `\`).
fn quote_exec(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for ch in value.chars() {
        match ch {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_paths_are_quoted() {
        assert_eq!(
            quote_exec("/usr/bin/radial-deck"),
            "\"/usr/bin/radial-deck\""
        );
        assert_eq!(
            quote_exec("/home/a b/radial-deck"),
            "\"/home/a b/radial-deck\""
        );
        assert_eq!(quote_exec("/tmp/a\"b"), "\"/tmp/a\\\"b\"");
    }
}
