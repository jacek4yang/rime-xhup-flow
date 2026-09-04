pub mod commands;
pub mod manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::product_status,
            commands::product_plan,
            commands::product_execute,
            commands::product_diagnostics,
            commands::learning_export,
            commands::learning_import,
            commands::learning_reset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
