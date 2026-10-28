//! External programs the tools and converters rely on.
//!
//! The domain layer owns the catalog so the GTK settings panel, the tray and
//! the headless `doctor` command all describe the same set of requirements.
//! Detection only looks at `PATH`; nothing is installed automatically.

use std::path::PathBuf;

use crate::convert::registry::find_tool;

/// One external program, described for humans and for package managers.
#[derive(Debug, Clone, Copy)]
pub struct Dependency {
    /// Stable identifier, also used in configuration and logs.
    pub id: &'static str,
    /// Name shown in the UI.
    pub label: &'static str,
    /// What stops working when the program is missing.
    pub purpose: &'static str,
    /// Executables that satisfy the requirement. Any one is enough.
    pub binaries: &'static [&'static str],
    /// Distribution package names.
    pub package: PackageNames,
}

#[derive(Debug, Clone, Copy)]
pub struct PackageNames {
    pub arch: &'static str,
    pub debian: &'static str,
    pub fedora: &'static str,
}

const CATALOG: &[Dependency] = &[
    Dependency {
        id: "imagemagick",
        label: "ImageMagick",
        purpose: "Image tools, collages, and PDF output",
        binaries: &["magick", "convert"],
        package: PackageNames {
            arch: "imagemagick",
            debian: "imagemagick",
            fedora: "ImageMagick",
        },
    },
    Dependency {
        id: "ffmpeg",
        label: "FFmpeg",
        purpose: "Video and audio conversions and tools",
        binaries: &["ffmpeg"],
        package: PackageNames {
            arch: "ffmpeg",
            debian: "ffmpeg",
            fedora: "ffmpeg",
        },
    },
    Dependency {
        id: "ghostscript",
        label: "Ghostscript",
        purpose: "PDF compression",
        binaries: &["gs"],
        package: PackageNames {
            arch: "ghostscript",
            debian: "ghostscript",
            fedora: "ghostscript",
        },
    },
    Dependency {
        id: "poppler",
        label: "Poppler",
        purpose: "PDF to image, text, and page tools",
        binaries: &["pdftoppm", "pdfunite", "pdfseparate"],
        package: PackageNames {
            arch: "poppler",
            debian: "poppler-utils",
            fedora: "poppler-utils",
        },
    },
    Dependency {
        id: "libreoffice",
        label: "LibreOffice",
        purpose: "Document, slide, and spreadsheet conversions",
        binaries: &["soffice", "libreoffice"],
        package: PackageNames {
            arch: "libreoffice-fresh",
            debian: "libreoffice",
            fedora: "libreoffice",
        },
    },
    Dependency {
        id: "pandoc",
        label: "Pandoc",
        purpose: "Markdown conversion",
        binaries: &["pandoc"],
        package: PackageNames {
            arch: "pandoc-cli",
            debian: "pandoc",
            fedora: "pandoc",
        },
    },
    Dependency {
        id: "7zip",
        label: "7-Zip",
        purpose: "Extracting archives the built-in reader cannot open",
        binaries: &["7z", "7zz"],
        package: PackageNames {
            arch: "7zip",
            debian: "7zip",
            fedora: "p7zip",
        },
    },
    Dependency {
        id: "kdeconnect",
        label: "KDE Connect",
        purpose: "Sending files to a phone",
        binaries: &["kdeconnect-cli"],
        package: PackageNames {
            arch: "kdeconnect",
            debian: "kdeconnect",
            fedora: "kdeconnect",
        },
    },
];

/// Every known dependency, in display order.
pub fn catalog() -> &'static [Dependency] {
    CATALOG
}

impl Dependency {
    pub fn lookup(id: &str) -> Option<&'static Self> {
        CATALOG.iter().find(|dependency| dependency.id == id)
    }

    /// Path of the first matching executable, if any is installed.
    pub fn executable(&self) -> Option<PathBuf> {
        self.binaries.iter().find_map(|name| find_tool(name))
    }

    pub fn is_available(&self) -> bool {
        self.executable().is_some()
    }
}

/// A catalog entry plus its current state.
#[derive(Debug, Clone, Copy)]
pub struct Status {
    pub dependency: &'static Dependency,
    pub available: bool,
}

/// Detects every dependency once. Callers that render often should cache this
/// and refresh on demand rather than probing the filesystem per frame.
pub fn statuses() -> Vec<Status> {
    CATALOG
        .iter()
        .map(|dependency| Status {
            dependency,
            available: dependency.is_available(),
        })
        .collect()
}

/// Package managers we know how to write an install line for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Pacman,
    Apt,
    Dnf,
    Zypper,
    Unknown,
}

impl PackageManager {
    pub fn detect() -> Self {
        for (binary, manager) in [
            ("pacman", Self::Pacman),
            ("apt", Self::Apt),
            ("dnf", Self::Dnf),
            ("zypper", Self::Zypper),
        ] {
            if find_tool(binary).is_some() {
                return manager;
            }
        }
        Self::Unknown
    }

    fn package(self, names: &PackageNames) -> &'static str {
        match self {
            Self::Pacman => names.arch,
            Self::Apt => names.debian,
            Self::Dnf | Self::Zypper => names.fedora,
            Self::Unknown => names.arch,
        }
    }

    /// Command a user can paste into a terminal, or `None` when the
    /// distribution is unknown.
    pub fn install_command(self, ids: &[&str]) -> Option<String> {
        if ids.is_empty() || self == Self::Unknown {
            return None;
        }
        let mut packages: Vec<&str> = Vec::new();
        for id in ids {
            let Some(dependency) = Dependency::lookup(id) else {
                continue;
            };
            let package = self.package(&dependency.package);
            if !packages.contains(&package) {
                packages.push(package);
            }
        }
        if packages.is_empty() {
            return None;
        }
        let joined = packages.join(" ");
        Some(match self {
            Self::Pacman => format!("sudo pacman -S --needed {joined}"),
            Self::Apt => format!("sudo apt install {joined}"),
            Self::Dnf => format!("sudo dnf install {joined}"),
            Self::Zypper => format!("sudo zypper install {joined}"),
            Self::Unknown => return None,
        })
    }
}

/// Missing dependency ids, in catalog order.
pub fn missing(statuses: &[Status]) -> Vec<&'static str> {
    statuses
        .iter()
        .filter(|status| !status.available)
        .map(|status| status.dependency.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_resolvable() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|entry| entry.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate dependency id");
        for dependency in CATALOG {
            assert!(Dependency::lookup(dependency.id).is_some());
            assert!(!dependency.binaries.is_empty());
        }
    }

    #[test]
    fn install_commands_use_the_right_package_names() {
        let command = PackageManager::Pacman
            .install_command(&["poppler", "7zip"])
            .unwrap();
        assert_eq!(command, "sudo pacman -S --needed poppler 7zip");
        let command = PackageManager::Apt
            .install_command(&["poppler", "7zip"])
            .unwrap();
        assert_eq!(command, "sudo apt install poppler-utils 7zip");
        assert!(PackageManager::Unknown
            .install_command(&["ffmpeg"])
            .is_none());
        assert!(PackageManager::Pacman.install_command(&[]).is_none());
    }

    #[test]
    fn install_commands_deduplicate_packages() {
        let command = PackageManager::Dnf
            .install_command(&["poppler", "poppler"])
            .unwrap();
        assert_eq!(command, "sudo dnf install poppler-utils");
    }

    #[test]
    fn statuses_cover_the_catalog() {
        let statuses = statuses();
        assert_eq!(statuses.len(), CATALOG.len());
        assert!(missing(&statuses).len() <= statuses.len());
    }
}
