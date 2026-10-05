//! Native container for the training-only application.
//!
//! Practice data and progress are owned by the frontend. This container exposes
//! no custom commands and never installs, detects or modifies a Rime profile.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
