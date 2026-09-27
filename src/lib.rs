mod commands;
mod models;

pub fn run() {
    tauri::Builder::default()
        .manage(commands::TerminalState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::list_dir,
            commands::file_info,
            commands::create_dir,
            commands::create_file,
            commands::delete_path,
            commands::rename_path,
            commands::copy_item,
            commands::move_item,
            commands::home_dir,
            commands::search_files,
            commands::duplicates_search,
            commands::batch_rename,
            commands::disk_usage,
            commands::recent_files,
            commands::hidden_files_setting,
            commands::get_hidden_setting,
            commands::background_setting,
            commands::get_background_setting,
            commands::pick_image,
            commands::load_image_data,
            commands::cli_open,
            commands::open_terminal,
            commands::preview_file,
            commands::compress_zip,
            commands::toggle_lock,
            commands::chmod_path,
            commands::path_exists,
            commands::read_file_text,
            commands::write_file_text,
            commands::visible_columns_setting,
            commands::get_visible_columns,
            commands::terminal_start,
            commands::terminal_read,
            commands::terminal_write,
            commands::terminal_kill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
