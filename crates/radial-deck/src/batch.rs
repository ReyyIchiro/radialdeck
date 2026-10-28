//! Headless conversion entry point.
//!
//! `radial-deck convert --to webp photo.png …` reuses the same engine as the
//! wheel, so scripting and the UI can never drift apart. This path never
//! initialises GTK: it is usable over SSH or in a Makefile.

use gtk::glib::ExitCode;
use std::path::PathBuf;

use deck_core::convert::registry::{self, Category};
use deck_core::{OutputLocation, Tools};

pub const HELP: &str = "\
USAGE:
    radial-deck convert --to <FORMAT> [--into <DIR> | --subfolder <NAME>] FILES…

Converts files with the same engine as the wheel. Outputs never overwrite an
existing file; a numeric suffix is added instead.

OPTIONS:
    --to <FORMAT>        Target extension, e.g. webp, mp4, pdf
    --into <DIR>         Write everything into this folder
    --subfolder <NAME>   Write into <source folder>/<NAME>
    --quiet              Only print errors
    -h, --help           Show this help
";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BatchArgs {
    pub target: Option<String>,
    pub into: Option<PathBuf>,
    pub subfolder: Option<String>,
    pub quiet: bool,
    pub help: bool,
    pub files: Vec<PathBuf>,
}

impl BatchArgs {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--to" | "-t" => {
                    let value = args.next().ok_or("--to needs a format")?;
                    parsed.target = Some(value.trim_start_matches('.').to_ascii_lowercase());
                }
                other if other.starts_with("--to=") => {
                    parsed.target = Some(
                        other["--to=".len()..]
                            .trim_start_matches('.')
                            .to_ascii_lowercase(),
                    );
                }
                "--into" | "-o" => {
                    let value = args.next().ok_or("--into needs a folder")?;
                    parsed.into = Some(PathBuf::from(value));
                }
                other if other.starts_with("--into=") => {
                    parsed.into = Some(PathBuf::from(&other["--into=".len()..]));
                }
                "--subfolder" => {
                    let value = args.next().ok_or("--subfolder needs a name")?;
                    parsed.subfolder = Some(value);
                }
                other if other.starts_with("--subfolder=") => {
                    parsed.subfolder = Some(other["--subfolder=".len()..].to_owned());
                }
                "--quiet" | "-q" => parsed.quiet = true,
                "--help" | "-h" => parsed.help = true,
                other if other.starts_with('-') => {
                    return Err(format!("unknown option: {other}"));
                }
                file => parsed.files.push(PathBuf::from(file)),
            }
        }
        if !parsed.help {
            if parsed.target.is_none() {
                return Err("--to is required".into());
            }
            if parsed.files.is_empty() {
                return Err("no input files".into());
            }
            if parsed.into.is_some() && parsed.subfolder.is_some() {
                return Err("--into and --subfolder cannot be combined".into());
            }
        }
        Ok(parsed)
    }

    fn output_location(&self) -> OutputLocation {
        if let Some(folder) = &self.into {
            OutputLocation::Folder(folder.clone())
        } else if let Some(name) = &self.subfolder {
            OutputLocation::Subfolder(name.clone())
        } else {
            OutputLocation::Alongside
        }
    }
}

/// Returns the process exit code: 0 only when every file converted.
pub fn run(args: BatchArgs) -> ExitCode {
    if args.help {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let Some(target) = args.target.clone() else {
        eprintln!("radial-deck: --to is required");
        return ExitCode::from(2);
    };
    let category = match common_category(&args.files) {
        Some(category) => category,
        None => {
            eprintln!("radial-deck: the inputs must all be the same kind of file");
            return ExitCode::from(2);
        }
    };
    let available = registry::targets(
        category,
        args.files.first().map(PathBuf::as_path),
        &Tools::detect(),
    );
    if !available.iter().any(|item| item.id == target) {
        let known: Vec<&str> = available
            .iter()
            .filter(|item| item.is_available())
            .map(|item| item.extension.as_str())
            .collect();
        eprintln!(
            "radial-deck: cannot convert {} to {target}. Available: {}",
            category.label(),
            known.join(", ")
        );
        return ExitCode::from(2);
    }

    let location = args.output_location();
    let outcome = deck_core::convert::convert_with(
        &args.files,
        category,
        &target,
        &Tools::detect(),
        location,
        deck_core::CancelToken::new(),
        std::sync::Arc::new(|_| {}),
    );
    if !args.quiet {
        for path in &outcome.created {
            println!("{}", path.display());
        }
    }
    for failure in &outcome.failures {
        eprintln!(
            "radial-deck: {}: {}",
            failure.source.display(),
            failure.message
        );
    }
    if outcome.failures.is_empty() && !outcome.created.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn common_category(files: &[PathBuf]) -> Option<Category> {
    let mut category: Option<Category> = None;
    for file in files {
        let found = Category::of(file)?;
        match category {
            None => category = Some(found),
            Some(existing) if existing == found => {}
            Some(_) => return None,
        }
    }
    category
}

/// True when the first argument selects the headless converter.
pub fn is_invocation(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("convert"))
}

pub fn strip_invocation(args: &[String]) -> Vec<String> {
    args.iter().skip(1).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<BatchArgs, String> {
        BatchArgs::parse(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn parses_target_and_files() {
        let args = parse(&["--to", ".WEBP", "a.png", "b.png"]).unwrap();
        assert_eq!(args.target.as_deref(), Some("webp"));
        assert_eq!(args.files.len(), 2);
        assert_eq!(args.output_location(), OutputLocation::Alongside);
    }

    #[test]
    fn parses_equals_form_and_output_folder() {
        let args = parse(&["--to=pdf", "--into=/tmp/out", "a.png"]).unwrap();
        assert_eq!(
            args.output_location(),
            OutputLocation::Folder("/tmp/out".into())
        );
    }

    #[test]
    fn subfolder_and_folder_are_mutually_exclusive() {
        assert!(parse(&["--to=pdf", "--into=/tmp", "--subfolder=out", "a.png"]).is_err());
    }

    #[test]
    fn target_and_inputs_are_required() {
        assert!(parse(&["a.png"]).is_err());
        assert!(parse(&["--to=pdf"]).is_err());
    }

    #[test]
    fn mixed_input_categories_are_rejected() {
        assert!(common_category(&["a.png".into(), "b.mp4".into()]).is_none());
        assert_eq!(
            common_category(&["a.png".into(), "b.jpg".into()]),
            Some(Category::Images)
        );
    }
}
