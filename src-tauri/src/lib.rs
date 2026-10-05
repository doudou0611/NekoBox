pub mod backend;
pub mod commands;
mod cover_protocol;
pub mod database;
pub mod domain;
mod screenshot_protocol;
mod updates;
mod window_material;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    #[cfg(target_os = "windows")]
    let context = window_material::configure(context);
    tauri::Builder::default()
        .register_uri_scheme_protocol("cover", |context, request| {
            let state = context.app_handle().try_state::<backend::BackendState>();
            let directory = state
                .as_ref()
                .and_then(|state| state.0.as_ref().ok())
                .map(|service| service.data_directory.as_path());
            #[cfg(target_os = "windows")]
            let origin = "http://tauri.localhost";
            #[cfg(not(target_os = "windows"))]
            let origin = "tauri://localhost";
            cover_protocol::response(directory, request.uri().path(), origin)
        })
        .register_uri_scheme_protocol("screenshot", |context, request| {
            #[cfg(target_os = "windows")]
            let origin = "http://tauri.localhost";
            #[cfg(not(target_os = "windows"))]
            let origin = "tauri://localhost";
            screenshot_protocol::response(
                context
                    .app_handle()
                    .try_state::<backend::BackendState>()
                    .as_deref(),
                request.uri().path(),
                origin,
            )
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::UpdateState::default())
        .setup(|app| {
            let show = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出 NekoBox").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| std::io::Error::other("未配置应用图标，无法创建系统托盘。"))?;
            TrayIconBuilder::with_id("main-tray")
                .menu(&menu)
                .icon(icon)
                .tooltip("NekoBox")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            let material = app
                .get_webview_window("main")
                .map(|window| window_material::apply(&window))
                .unwrap_or(window_material::WindowMaterial::Solid);
            app.manage(material);
            let backend = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|p| p.join("data")))
                .ok_or(backend::ServiceError(
                    domain::ErrorCode::PathInvalid,
                    "无法确定 exe 旁的数据目录。",
                ))
                .and_then(backend::Backend::open);
            if let Ok(service) = &backend {
                backend::application_backup::scheduler(service);
                let covers = service.data_directory.join("covers");
                std::fs::create_dir_all(&covers)?;
                app.asset_protocol_scope().allow_directory(&covers, false)?;
            }
            app.manage(backend::BackendState(backend));
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                let material = webview
                    .app_handle()
                    .try_state::<window_material::WindowMaterial>()
                    .map(|state| *state)
                    .unwrap_or(window_material::WindowMaterial::Solid);
                // This is a host-owned startup marker, not a business IPC/event or an OS visual proof.
                if webview.eval(material.script()).is_err() {
                    eprintln!("无法设置窗口材质标记，网页保持不透明回退。");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::hikarifield_account,
            commands::login_hikarifield,
            commands::logout_hikarifield,
            commands::sync_hikarifield,
            commands::get_hikarifield_settings,
            commands::set_hikarifield_path,
            commands::start_hikarifield_download,
            commands::list_hikarifield_downloads,
            commands::cancel_hikarifield_download,
            commands::health_check,
            commands::check_app_update,
            commands::get_app_update_status,
            commands::download_app_update,
            commands::install_app_update,
            commands::open_app_update_release,
            commands::get_app_settings,
            commands::cache_remote_image,
            commands::open_bangumi_login,
            commands::start_metadata_refresh,
            commands::get_metadata_refresh,
            commands::cancel_metadata_refresh,
            commands::get_backup_settings,
            commands::save_backup_settings,
            commands::create_application_backup,
            commands::list_application_backups,
            commands::delete_application_backup,
            commands::preview_application_restore,
            commands::confirm_application_restore,
            commands::cancel_application_restore,
            commands::get_application_backup_status,
            commands::test_webdav,
            commands::list_webdav_backups,
            commands::upload_webdav_backup,
            commands::download_webdav_backup,
            commands::save_app_settings,
            commands::backend_status,
            commands::export_database,
            commands::preview_database_import,
            commands::confirm_database_import,
            commands::cancel_database_import,
            commands::database_transfer_status,
            commands::scan_roots,
            commands::get_scan_task,
            commands::control_scan_task,
            commands::list_games,
            commands::get_game,
            commands::import_game,
            commands::preview_import,
            commands::begin_import_batch,
            commands::cancel_import_batch,
            commands::prepare_import_metadata,
            commands::hikarinagi_account,
            commands::begin_hikarinagi_login,
            commands::poll_hikarinagi_login,
            commands::cancel_hikarinagi_login,
            commands::logout_hikarinagi,
            commands::get_metadata_sources,
            commands::save_metadata_sources,
            commands::get_vndb_settings,
            commands::save_vndb_settings,
            commands::test_vndb_connection,
            commands::sync_account_play_data,
            commands::get_hikarinagi_settings,
            commands::get_hikarinagi_rates,
            commands::get_hikarinagi_review,
            commands::submit_hikarinagi_review,
            commands::save_hikarinagi_settings,
            commands::test_hikarinagi_connection,
            commands::discard_import_metadata,
            commands::import_prepared_game,
            commands::update_game,
            commands::update_game_metadata,
            commands::set_metadata_lock,
            commands::list_game_processes,
            commands::select_game_process,
            commands::remove_game,
            commands::get_installation,
            commands::configure_installation,
            commands::launch_game,
            commands::list_play_sessions,
            commands::correct_play_session,
            commands::get_playtime_stats,
            commands::get_activity_snapshot,
            commands::detect_save_paths,
            commands::list_save_profiles,
            commands::configure_save_profile,
            commands::delete_save_profile,
            commands::list_save_snapshots,
            commands::create_save_snapshot,
            commands::preview_save_restore,
            commands::restore_save_snapshot,
            commands::delete_save_snapshot,
            commands::export_screenshots,
            commands::delete_screenshots,
            commands::scan_screenshots,
            commands::list_screenshots,
            commands::update_screenshot,
            commands::list_notes,
            commands::save_note,
            commands::delete_note,
            commands::replace_game_tags,
            commands::list_external_sources,
            commands::bind_external_source,
            commands::open_external_source,
            commands::get_recommendations,
            commands::set_recommendation_preference,
            commands::list_recommendation_preferences,
            commands::get_home_summary,
            commands::list_collections,
            commands::reorder_collections,
            commands::get_collection,
            commands::save_collection,
            commands::set_collection_members,
            commands::delete_collection,
            commands::search_metadata,
            commands::cancel_metadata_search,
            commands::confirm_metadata_match,
            commands::get_translation_settings,
            commands::save_translation_settings,
            commands::test_translation,
            commands::unbind_metadata,
            commands::bangumi_account,
            commands::login_bangumi,
            commands::logout_bangumi,
            commands::get_bangumi_cover_status,
            commands::retry_bangumi_cover
        ])
        .run(context)
        .expect("error while running NekoBox");
}
