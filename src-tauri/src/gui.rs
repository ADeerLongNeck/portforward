//! GUI mode using Tauri

use crate::commands::config_cmd::ConfigState;
use crate::commands::stats_cmd::StatsState;
use crate::commands::tunnel_cmd::RuntimeState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Port Forward application (GUI mode)");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // State management
        .manage(ConfigState::new())
        .manage(RuntimeState::new())
        .manage(StatsState::new())
        // Register commands
        .invoke_handler(tauri::generate_handler![
            // Config commands
            crate::commands::config_cmd::get_config,
            crate::commands::config_cmd::save_config,
            // Tunnel commands
            crate::commands::tunnel_cmd::test_connection,
            crate::commands::tunnel_cmd::start_server,
            crate::commands::tunnel_cmd::stop_server,
            crate::commands::tunnel_cmd::start_client,
            crate::commands::tunnel_cmd::stop_client,
            crate::commands::tunnel_cmd::get_status,
            crate::commands::tunnel_cmd::get_forwarded_ports,
            // Stats commands
            crate::commands::stats_cmd::get_stats,
            crate::commands::stats_cmd::update_stats,
        ])
        .setup(|_app| {
            #[cfg(debug_assertions)]
            {
                // let window = app.get_webview_window("main").unwrap();
                // window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
