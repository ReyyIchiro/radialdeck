mod actions;
mod autostart;
mod controller;
mod hotkey;
mod launcher;
mod manage;
mod settings;
mod theme;
mod tray;
mod wheel;

use gtk::prelude::*;

pub(crate) const APP_ID: &str = "io.github.raihan.RadialDeck";

const HELP: &str = "\
RadialDeck — radial overlay launcher for project folders

USAGE:
    radial-deck [OPTIONS]

OPTIONS:
    --hidden          Start in the tray without showing the overlay
    --show            Show the overlay immediately on start
    --manage          Open the folder management window
    --settings        Open the settings window
    --reset-config    Delete the saved configuration, then start with defaults
    -V, --version     Print version and exit
    -h, --help        Print this help and exit

ENVIRONMENT:
    RUST_LOG          Log filter (e.g. radial_deck=debug)
";

fn main() -> gtk::glib::ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("radial_deck=info,warn")),
        )
        .with_writer(std::io::stderr)
        .try_init();

    let args: Vec<String> = std::env::args().collect();
    let cli = controller::Cli::parse(args.iter().skip(1).cloned());

    if cli.help {
        println!("{HELP}");
        return gtk::glib::ExitCode::SUCCESS;
    }
    if cli.version {
        println!("radial-deck {}", env!("CARGO_PKG_VERSION"));
        return gtk::glib::ExitCode::SUCCESS;
    }

    let app = gtk::Application::builder().application_id(APP_ID).build();
    let controller = controller::Controller::new(cli);

    {
        let controller = controller.clone();
        app.connect_startup(move |app| {
            theme::install_css();
            controller.start(app);
        });
    }
    {
        let controller = controller.clone();
        app.connect_activate(move |_| {
            controller.handle_activate();
        });
    }

    let program = args.first().map(String::as_str).unwrap_or("radial-deck");
    app.run_with_args(&[program])
}
