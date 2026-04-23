mod commands;
mod database;
mod error;
mod proxy;

use database::Database;
use proxy::ProxyServer;
use std::sync::Arc;
use tauri::Manager;

pub use error::AppError;

/// Shared application state
pub struct AppState {
    pub db: Arc<Database>,
    pub proxy: Arc<tokio::sync::RwLock<Option<ProxyServer>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Initialize database
            let db = Database::open()?;
            db.create_tables()?;

            let state = AppState {
                db: Arc::new(db),
                proxy: Arc::new(tokio::sync::RwLock::new(None)),
            };
            app.manage(state);

            log::info!("API Switch initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Channel
            commands::channel::list_channels,
            commands::channel::create_channel,
            commands::channel::update_channel,
            commands::channel::delete_channel,
            commands::channel::fetch_models,
            commands::channel::select_models,
            // API Pool
            commands::pool::list_entries,
            commands::pool::toggle_entry,
            commands::pool::reorder_entries,
            commands::pool::create_entry,
            // Access Keys
            commands::token::list_access_keys,
            commands::token::create_access_key,
            commands::token::delete_access_key,
            commands::token::toggle_access_key,
            // Usage
            commands::usage::get_usage_logs,
            commands::usage::get_dashboard_stats,
            commands::usage::get_model_consumption,
            commands::usage::get_call_trend,
            commands::usage::get_model_distribution,
            commands::usage::get_model_ranking,
            commands::usage::get_user_ranking,
            commands::usage::get_user_trend,
            // Config
            commands::config::get_settings,
            commands::config::update_settings,
            // Proxy
            commands::proxy_cmd::start_proxy,
            commands::proxy_cmd::stop_proxy,
            commands::proxy_cmd::get_proxy_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
