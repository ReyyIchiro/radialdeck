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
