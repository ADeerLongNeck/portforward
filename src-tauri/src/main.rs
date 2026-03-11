#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    #[cfg(feature = "gui")]
    {
        port_forward_tauri::run();
    }

    #[cfg(feature = "cli")]
    {
        // CLI mode
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = port_forward_tauri::cli::run_cli().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        });
    }
}
