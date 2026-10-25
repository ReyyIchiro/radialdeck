mod action_launcher;
mod action_wheel;
mod actions;
mod autostart;
mod controller;
mod float_shelf;
mod hotkey;
mod launcher;
mod logging;
mod manage;
mod project_tabs;
mod settings;
mod surface;
mod theme;
mod tray;
mod wheel;
mod widgets;

use gtk::prelude::*;

pub(crate) const APP_ID: &str = "io.github.raihan.RadialDeck";

const HELP: &str = "\
RadialDeck — radial file action wheel for the Linux desktop

USAGE:
    radial-deck [OPTIONS] [FILES…]

OPTIONS:
    --hidden          Start in the tray without showing the wheel
    --show            Show the wheel immediately
    --projects        Open the project-folder wheel immediately
    --manage          Open the folder management window
    --settings        Open the settings window
    --reset-config    Delete the saved configuration, then start with defaults
    -V, --version     Print version and exit
    -h, --help        Print this help and exit

FILES are opened in the wheel, e.g. from a file manager service menu.

ENVIRONMENT:
    RUST_LOG          Log filter (e.g. radial_deck=debug)

Logs are written to $XDG_STATE_HOME/radial-deck/logs (7 daily files).
";

fn main() -> gtk::glib::ExitCode {
    // The UI is small, static 2D; Cairo keeps idle memory far below the GL
    // renderer and avoids relying on the Optimus GPU. Users can override it.
    if std::env::var_os("GSK_RENDERER").is_none() {
        std::env::set_var("GSK_RENDERER", "cairo");
    }

    let _log_guard = logging::init();
    if let Some(dir) = logging::log_dir() {
        tracing::debug!("log directory: {}", dir.display());
    }

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
    app.set_flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE);
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
    {
        let controller = controller.clone();
        app.connect_command_line(move |app, command_line| {
            let arguments = command_line.arguments();
            tracing::debug!("command line: {arguments:?}");
            let cli = controller::Cli::parse(
                arguments
                    .into_iter()
                    .skip(1)
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );
            controller.handle_cli(cli);
            app.activate();
            gtk::glib::ExitCode::SUCCESS
        });
    }

    if args.is_empty() {
        app.run()
    } else {
        app.run_with_args(&args)
    }
}
