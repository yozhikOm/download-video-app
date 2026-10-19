mod commands;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::fetch_info,
            commands::start_download,
            commands::cancel_download,
            commands::download_transcript,
            commands::ytdlp_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
