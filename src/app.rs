//! Application lifecycle and main window initialization
//!
//! Sets up the Libadwaita Application (`com.shadhin.browser`), establishes
//! the window, and embeds a `webkit6::WebView` loading a verified URL.

use crate::browser::cli::{format_help_text, parse_cli_args};
use crate::ui;
use crate::ui::window::ShadhinWindow;
use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;
use tracing::info;

pub const APP_ID: &str = "com.shadhin.browser";
pub const APP_NAME: &str = "Shadhin Browser";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct ShadhinApp {
    app: adw::Application,
}

impl Default for ShadhinApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadhinApp {
    /// Creates a new `ShadhinApp` instance configured with the official application ID
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id(APP_ID)
            .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build();

        app.connect_command_line(Self::on_command_line);
        app.connect_activate(Self::on_activate);

        Self { app }
    }

    /// Command-line argument handler (handles version, help, private mode, and URLs)
    fn on_command_line(
        app: &adw::Application,
        cmdline: &gio::ApplicationCommandLine,
    ) -> glib::ExitCode {
        let args: Vec<String> = cmdline
            .arguments()
            .into_iter()
            .filter_map(|s| s.into_string().ok())
            .collect();

        match parse_cli_args(&args) {
            Ok(opts) => {
                if opts.show_version {
                    println!("{} {}", APP_NAME, APP_VERSION);
                    return glib::ExitCode::SUCCESS;
                }

                if opts.show_help {
                    println!("{}", format_help_text(APP_NAME, APP_VERSION));
                    return glib::ExitCode::SUCCESS;
                }

                if opts.private_window {
                    Self::launch_private(app);
                    return glib::ExitCode::SUCCESS;
                }

                let windows = app.windows();
                if opts.new_window || windows.is_empty() {
                    Self::launch(app, &opts.urls);
                } else if !opts.urls.is_empty() {
                    // Open requested URLs in a new browser window or existing instance
                    Self::launch(app, &opts.urls);
                } else if let Some(first_win) = windows.first() {
                    first_win.present();
                }

                glib::ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("Error: {err}\nRun 'shadhin-browser --help' for usage.");
                glib::ExitCode::FAILURE
            }
        }
    }

    /// Primary activation signal handler
    fn on_activate(app: &adw::Application) {
        Self::launch(app, &[]);
    }

    /// Prepares UI styling and decoration layout
    fn prepare_environment() {
        // Force Libadwaita dark scheme as per UI specification 2.1
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

        // Ensure minimize, maximize, and close buttons are shown (overriding WSLg defaults)
        if let Some(settings) = gtk4::Settings::default() {
            settings.set_gtk_decoration_layout(Some(":minimize,maximize,close"));
        }

        // Load GResource & apply CSS theme
        ui::setup_theme();
    }

    /// Launches a private window
    fn launch_private(app: &adw::Application) {
        info!(
            "Launching {} ({}) in Private Browsing Mode",
            APP_NAME, APP_ID
        );
        Self::prepare_environment();
        let window = ShadhinWindow::new_private(app);
        window.present();
    }

    /// Launches the main browser window with optional URLs
    fn launch(app: &adw::Application, urls: &[String]) {
        info!(
            "Launching {} ({}) with {} URLs",
            APP_NAME,
            APP_ID,
            urls.len()
        );

        Self::prepare_environment();

        // Build main application window shell
        let window = ShadhinWindow::new_with_urls(app, urls);
        window.present();

        if std::env::var("SHADHIN_BENCH_STARTUP").is_ok() {
            let app_clone = app.clone();
            glib::idle_add_local_once(move || {
                info!("STARTUP_BENCHMARK_COMPLETE");
                app_clone.quit();
            });
        }
    }

    /// Runs the application GLib/GIO main loop with command-line arguments
    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}
