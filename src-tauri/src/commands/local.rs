use crate::{
    backend::{self, types::*, BackendState},
    domain::{
        models::*,
        protocol::{valid_request_id, ErrorCode},
        requests::*,
        ApiRequest, ApiResponse,
    },
};
use serde::Serialize;
use tauri::Manager;

fn resolve(app: &tauri::AppHandle) -> backend::Result<backend::Backend> {
    app.state::<BackendState>().0.clone()
}

async fn dispatch<P: Send + 'static, T: Serialize + Send + 'static>(
    state: backend::Result<backend::Backend>,
    request: ApiRequest<P>,
    operation: impl FnOnce(backend::Backend, P, &str) -> backend::Result<T> + Send + 'static,
) -> ApiResponse<T> {
    let request_id = request.request_id;
    if !valid_request_id(&request_id) {
        return ApiResponse::error(String::new(), ErrorCode::InvalidRequest, "请求标识无效。");
    }
    let backend = match state {
        Ok(b) => b,
        Err(e) => return ApiResponse::error(request_id, e.0, e.1),
    };
    let task_id = request_id.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        let service = backend.clone();
        let _operation = service
            .operation_gate
            .read()
            .map_err(|_| backend::invalid("应用正在安装更新。"))?;
        operation(backend, request.payload, &task_id)
    })
    .await
    {
        Ok(Ok(data)) => ApiResponse::ok(request_id, data, "操作完成。"),
        Ok(Err(e)) => ApiResponse::error(request_id, e.0, e.1),
        Err(_) => ApiResponse::error(
            request_id,
            ErrorCode::InternalError,
            "服务执行失败，请重新启动应用。",
        ),
    }
}
macro_rules! command {
    ($name:ident,$payload:ty,$result:ty,$operation:expr) => {
        #[tauri::command]
        pub async fn $name(
            app: tauri::AppHandle,
            request: ApiRequest<$payload>,
        ) -> ApiResponse<$result> {
            dispatch(resolve(&app), request, $operation).await
        }
    };
}
command!(backend_status, EmptyRequest, BackendStatus, |b, _, _| b
    .status());
command!(
    list_play_sessions,
    backend::playtime::SessionQuery,
    Paginated<backend::playtime::PlaySession>,
    |b, q, _| b.database()?.list_play_sessions(&q)
);
command!(
    correct_play_session,
    backend::playtime::CorrectSessionRequest,
    backend::playtime::PlaySession,
    |b, q, _| b.database()?.correct_play_session(&q)
);
command!(
    list_recommendation_preferences,
    EmptyRequest,
    Vec<RecommendationPreferenceEntry>,
    |b, _, _| b.database()?.recommendation_preferences()
);
command!(
    get_activity_snapshot,
    backend::activity::ActivityQuery,
    backend::activity::ActivitySnapshot,
    |b, q, _| b.database()?.activity_snapshot(&q)
);
command!(
    get_playtime_stats,
    backend::playtime::StatsQuery,
    backend::playtime::PlaytimeStats,
    |b, q, _| b.database()?.playtime_stats(&q)
);
command!(
    detect_save_paths,
    InstallIdRequest,
    Vec<String>,
    |b, q, _| b.detect_save_paths(&q.install_id)
);
command!(
    list_save_profiles,
    GameIdRequest,
    Vec<backend::saves::types::SaveProfile>,
    |b, q, _| b.list_save_profiles(&q.game_id)
);
command!(
    configure_save_profile,
    backend::saves::types::ConfigureProfile,
    backend::saves::types::SaveProfile,
    |b, q, _| b.configure_save_profile(q)
);
command!(
    list_save_snapshots,
    GameIdRequest,
    Vec<SaveSnapshot>,
    |b, q, _| b.database()?.save_snapshots(&q.game_id)
);
command!(
    delete_save_profile,
    backend::saves::types::DeleteProfile,
    bool,
    |b, q, _| b.delete_save_profile(q)
);
command!(
    preview_save_restore,
    backend::saves::types::SnapshotRequest,
    backend::saves::types::RestorePreview,
    |b, q, _| b.preview_save_restore(&q.snapshot_id)
);
command!(
    delete_save_snapshot,
    backend::saves::types::DeleteSnapshot,
    bool,
    |b, q, _| b.delete_save_snapshot(q)
);
command!(
    scan_screenshots,
    backend::types::ScanScreenshotsRequest,
    u64,
    |b, q, _| b.scan_screenshots(q)
);
command!(
    list_screenshots,
    backend::types::ScreenshotQuery,
    Vec<Screenshot>,
    |b, q, _| b.list_screenshots(q)
);
command!(
    update_screenshot,
    UpdateScreenshotRequest,
    Screenshot,
    |b, q, _| b.update_screenshot(&q)
);
command!(list_notes, GameIdRequest, Vec<Note>, |b, q, _| b
    .list_notes(&q.game_id));
command!(save_note, backend::types::NoteRequest, Note, |b, q, _| b
    .save_note(&q));
command!(
    delete_note,
    backend::types::DeleteNoteRequest,
    bool,
    |b, q, _| b.delete_note(&q.id, q.confirmed)
);
command!(
    replace_game_tags,
    backend::types::ReplaceGameTagsRequest,
    Vec<Tag>,
    |b, q, _| b.replace_game_tags(&q)
);
#[tauri::command]
pub async fn create_save_snapshot(
    app: tauri::AppHandle,
    request: ApiRequest<backend::saves::types::CreateSnapshot>,
) -> ApiResponse<SaveSnapshot> {
    dispatch(resolve(&app), request, move |b, q, id| {
        b.create_save_snapshot(q, id, Some(&app))
    })
    .await
}
#[tauri::command]
pub async fn restore_save_snapshot(
    app: tauri::AppHandle,
    request: ApiRequest<RestoreSaveSnapshotRequest>,
) -> ApiResponse<RestoreResult> {
    dispatch(resolve(&app), request, move |b, q, id| {
        b.restore_save_snapshot(q, id, Some(&app))
    })
    .await
}
command!(
    bangumi_account,
    EmptyRequest,
    backend::bangumi_account::Account,
    |b, _, _| backend::bangumi_account::account(&b)
);
command!(
    login_bangumi,
    backend::bangumi_account::LoginRequest,
    backend::bangumi_account::Account,
    |b, q, _| backend::bangumi_account::login(&b, q)
);
command!(
    logout_bangumi,
    EmptyRequest,
    backend::bangumi_account::Account,
    |b, _, _| backend::bangumi_account::logout(&b)
);
command!(
    get_bangumi_cover_status,
    GameIdRequest,
    backend::bangumi::CoverStatus,
    |b, q, _| backend::bangumi::cover_status(&b, &q.game_id)
);
command!(
    retry_bangumi_cover,
    backend::bangumi::RetryRequest,
    backend::bangumi::CoverStatus,
    |b, q, _| backend::bangumi::retry(&b, q)
);
command!(list_games, GameQuery, Paginated<GameSummary>, |b, q, _| {
    let active = b
        .active_playtime
        .lock()
        .map_err(|_| backend::invalid("计时服务不可用。"))?
        .clone();
    let db = b.database()?;
    let mut result = db.list_games(&q)?;
    let extra = db.live_playtime_extra(&active)?;
    for game in &mut result.items {
        game.total_playtime_seconds += extra.get(&game.id).copied().unwrap_or(0);
    }
    Ok(result)
});
command!(get_game, GameIdRequest, GameDetail, |b, q, _| {
    let active = b
        .active_playtime
        .lock()
        .map_err(|_| backend::invalid("计时服务不可用。"))?
        .clone();
    let db = b.database()?;
    let mut game = db.get_game(&q.game_id)?;
    game.summary.total_playtime_seconds += db
        .live_playtime_extra(&active)?
        .get(&q.game_id)
        .copied()
        .unwrap_or(0);
    Ok(game)
});
command!(import_game, ImportGameRequest, GameDetail, |b, q, _| {
    backend::scanner::manual_import(&b, &q)
});
command!(
    prepare_import_metadata,
    backend::import_metadata::PrepareRequest,
    backend::import_metadata::Preparation,
    |b, q, _| backend::import_metadata::prepare(&b, q)
);
command!(
    get_hikarinagi_settings,
    EmptyRequest,
    backend::hikarinagi_settings::SettingsView,
    |b, _, _| backend::hikarinagi_settings::get_settings(&b)
);
command!(
    save_hikarinagi_settings,
    backend::hikarinagi_settings::SaveRequest,
    backend::hikarinagi_settings::SettingsView,
    |b, q, _| backend::hikarinagi_settings::save_settings(&b, q)
);
command!(test_hikarinagi_connection, EmptyRequest, bool, |b, _, _| {
    backend::hikarinagi::test_connection(&b)
});
command!(
    get_hikarinagi_rates,
    backend::hikarinagi_rates::Request,
    backend::hikarinagi_rates::Response,
    |b, q, _| backend::hikarinagi_rates::get(&b, q)
);
command!(
    get_hikarinagi_review,
    backend::hikarinagi_review::GetRequest,
    backend::hikarinagi_review::GetResponse,
    |b, q, _| backend::hikarinagi_review::get(&b, q)
);
command!(
    submit_hikarinagi_review,
    backend::hikarinagi_review::SubmitRequest,
    backend::hikarinagi_review::SubmitResponse,
    |b, q, _| backend::hikarinagi_review::submit(&b, q)
);
command!(
    discard_import_metadata,
    backend::import_metadata::DiscardRequest,
    bool,
    |b, q, _| backend::import_metadata::discard(&b, q)
);
command!(
    import_prepared_game,
    backend::import_metadata::CommitRequest,
    GameDetail,
    |b, q, _| backend::import_metadata::commit(&b, q)
);
command!(
    preview_import,
    PreviewImportRequest,
    ImportPreviewReport,
    |b, q, _| backend::scanner::preview_import(&b, &q)
);
command!(update_game, UpdateGameRequest, GameDetail, |b, q, _| b
    .database()?
    .update_game(&q));
command!(remove_game, RemoveRecordRequest, bool, |b, q, _| {
    if !q.confirmed {
        return Err(backend::invalid("移除库记录需要确认。"));
    }
    b.remove_record(&q.id, false)
});
command!(
    get_installation,
    InstallIdRequest,
    InstallationDetails,
    |b, q, _| b.database()?.installation(&q.install_id)
);
command!(
    configure_installation,
    ConfigureInstallationRequest,
    InstallationDetails,
    |b, q, _| b.configure_installation(&q)
);
command!(get_scan_task, TaskIdRequest, ScanReport, |b, q, _| b
    .scan_report(&q.task_id));
command!(
    control_scan_task,
    ControlScanRequest,
    ScanReport,
    |b, q, _| b.control_scan(&q)
);
command!(
    get_home_summary,
    crate::domain::home::HomeQuery,
    HomeSummary,
    |b, q, _| b.database()?.home_summary_window(&q)
);
command!(
    list_collections,
    EmptyRequest,
    Vec<CollectionSummary>,
    |b, _, _| b.database()?.list_collections()
);
command!(
    get_collection,
    CollectionIdRequest,
    CollectionDetail,
    |b, q, _| b.database()?.get_collection(&q.collection_id)
);
command!(
    reorder_collections,
    ReorderCollectionsRequest,
    bool,
    |b, q, _| b.database()?.reorder_collections(&q.collection_ids)
);
command!(
    save_collection,
    SaveCollectionRequest,
    CollectionDetail,
    |b, q, _| b.database()?.save_collection(&q)
);
command!(
    set_collection_members,
    SetCollectionMembersRequest,
    CollectionDetail,
    |b, q, _| b.database()?.replace_members(&q.collection_id, &q.game_ids)
);
command!(delete_collection, RemoveRecordRequest, bool, |b, q, _| {
    if !q.confirmed {
        return Err(backend::invalid("删除分组需要确认。"));
    }
    b.remove_record(&q.id, true)
});
command!(
    search_metadata,
    SearchMetadataRequest,
    Vec<MetadataCandidate>,
    |b, q, request_id| {
        let scope = backend::metadata_search::begin(&b, request_id)?;
        backend::metadata_sources::search(&scope.backend, &q)
    }
);
command!(
    cancel_metadata_search,
    backend::metadata_search::CancelRequest,
    bool,
    |b, q, _| backend::metadata_search::cancel(&b, q)
);
command!(
    confirm_metadata_match,
    ConfirmMetadataMatchRequest,
    MatchResult,
    |b, q, _| backend::translation::confirm(&b, &q)
);
command!(unbind_metadata, UnbindMetadataRequest, bool, |b, q, _| {
    b.unbind_metadata(&q).map(|_| true)
});
command!(
    list_external_sources,
    backend::types::ExternalSourcesRequest,
    Vec<ExternalSource>,
    |b, q, _| b.list_external_sources(q)
);
command!(
    get_recommendations,
    RecommendationQuery,
    Vec<Recommendation>,
    |b, q, _| b.database()?.recommendations(&q)
);
command!(
    set_recommendation_preference,
    SetRecommendationPreferenceRequest,
    PreferenceResult,
    |b, q, _| {
        let mut database = b.database()?;
        database.set_recommendation_preference(&q)
    }
);
#[tauri::command]
pub async fn scan_roots(
    app: tauri::AppHandle,
    request: ApiRequest<ScanRootsRequest>,
) -> ApiResponse<ScanTask> {
    dispatch(resolve(&app), request, move |b, q, id| {
        b.start_scan(&q, id, Some(app))
    })
    .await
}
#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    request: ApiRequest<LaunchGameRequest>,
) -> ApiResponse<LaunchSession> {
    dispatch(resolve(&app), request, move |b, q, id| {
        b.launch(&q, id, Some(app))
    })
    .await
}

command!(
    bind_external_source,
    BindSourceRequest,
    Vec<ExternalSource>,
    |b, q, _| b.bind_external_source(q)
);
command!(open_external_source, OpenSourceRequest, bool, |b, q, _| b
    .open_external_source(q));

command!(
    export_database,
    backend::transfer::ExportRequest,
    backend::transfer::ExportResult,
    |b, q, _| b.export_database(q)
);
command!(
    preview_database_import,
    backend::transfer::PreviewRequest,
    backend::transfer::ImportPreview,
    |b, q, _| b.preview_database_import(q)
);
command!(
    confirm_database_import,
    backend::transfer::ConfirmRequest,
    backend::transfer::TransferStatus,
    |b, q, _| b.confirm_database_import(q)
);
command!(
    cancel_database_import,
    EmptyRequest,
    backend::transfer::TransferStatus,
    |b, _, _| b.cancel_database_import()
);
command!(
    database_transfer_status,
    EmptyRequest,
    backend::transfer::TransferStatus,
    |b, _, _| b.database_transfer_status()
);

command!(
    get_translation_settings,
    EmptyRequest,
    backend::translation::SettingsView,
    |b, _, _| backend::translation::get_settings(&b)
);
command!(
    save_translation_settings,
    backend::translation::SaveRequest,
    backend::translation::SettingsView,
    |b, q, _| backend::translation::save_settings(&b, q)
);
command!(
    test_translation,
    EmptyRequest,
    backend::translation::TestResult,
    |b, _, _| backend::translation::test_translation(&b)
);

command!(
    hikarinagi_account,
    EmptyRequest,
    backend::hikarinagi_account::Account,
    |b, _, _| backend::hikarinagi_account::account(&b)
);
#[tauri::command]
pub async fn begin_hikarinagi_login(
    app: tauri::AppHandle,
    request: ApiRequest<EmptyRequest>,
) -> ApiResponse<backend::hikarinagi_account::Begin> {
    let handle = app.clone();
    dispatch(resolve(&app), request, move |b, _, _| {
        let flow = backend::hikarinagi_account::begin(&b)?;
        let label = format!("hikarinagi-login-{}", flow.flow_id);
        let url = flow
            .authorization_url
            .parse()
            .map_err(|_| backend::invalid("官方授权地址无效。"))?;
        let builder =
            tauri::WebviewWindowBuilder::new(&handle, &label, tauri::WebviewUrl::External(url))
                .title("登录 Hikarinagi")
                .inner_size(920.0, 760.0)
                .min_inner_size(640.0, 480.0)
                .visible(true);
        let creation = login_proxy(&b, builder)?.build();
        match creation {
            Ok(window) => {
                let cleanup = b.clone();
                let flow_id = flow.flow_id.clone();
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Destroyed) {
                        let _ = backend::hikarinagi_account::cancel(
                            &cleanup,
                            backend::hikarinagi_account::FlowRequest {
                                flow_id: flow_id.clone(),
                            },
                        );
                    }
                });
                if window.show().and_then(|_| window.set_focus()).is_err() {
                    let _ = backend::hikarinagi_account::cancel(
                        &b,
                        backend::hikarinagi_account::FlowRequest {
                            flow_id: flow.flow_id,
                        },
                    );
                    let _ = window.close();
                    return Err(backend::permission(
                        "无法显示或激活官方授权窗口，请重新登录。",
                    ));
                }
                Ok(flow)
            }
            Err(_) => {
                let _ = backend::hikarinagi_account::cancel(
                    &b,
                    backend::hikarinagi_account::FlowRequest {
                        flow_id: flow.flow_id,
                    },
                );
                Err(backend::permission(
                    "无法创建官方授权窗口，请检查 WebView2 后重新登录。",
                ))
            }
        }
    })
    .await
}

command!(
    poll_hikarinagi_login,
    backend::hikarinagi_account::FlowRequest,
    backend::hikarinagi_account::LoginFlow,
    |b, q, _| backend::hikarinagi_account::poll(&b, q)
);
command!(
    cancel_hikarinagi_login,
    backend::hikarinagi_account::FlowRequest,
    bool,
    |b, q, _| backend::hikarinagi_account::cancel(&b, q)
);
command!(
    logout_hikarinagi,
    EmptyRequest,
    backend::hikarinagi_account::Account,
    |b, _, _| backend::hikarinagi_account::logout(&b)
);
command!(
    get_vndb_settings,
    EmptyRequest,
    backend::vndb_settings::Settings,
    |b, _, _| backend::vndb_settings::get(&b)
);
command!(
    save_vndb_settings,
    backend::vndb_settings::SaveRequest,
    backend::vndb_settings::Settings,
    |b, q, _| backend::vndb_settings::save(&b, q)
);
command!(
    test_vndb_connection,
    EmptyRequest,
    backend::vndb_settings::Connection,
    |b, _, _| backend::vndb_settings::test(&b)
);
command!(
    sync_account_play_data,
    backend::account_sync::Request,
    backend::account_sync::Report,
    |b, q, _| backend::account_sync::sync(&b, q)
);

command!(
    get_metadata_sources,
    EmptyRequest,
    backend::metadata_sources::Config,
    |b, _, _| backend::metadata_sources::get(&b)
);
command!(
    save_metadata_sources,
    backend::metadata_sources::Config,
    backend::metadata_sources::Config,
    |b, q, _| backend::metadata_sources::save(&b, q)
);

command!(
    begin_import_batch,
    EmptyRequest,
    backend::import_metadata::BeginBatch,
    |b, _, _| backend::import_metadata::begin_batch(&b)
);
command!(
    cancel_import_batch,
    backend::import_metadata::CancelBatch,
    bool,
    |b, q, _| backend::import_metadata::cancel_batch(&b, q)
);

#[tauri::command]
pub async fn export_screenshots(
    app: tauri::AppHandle,
    request: ApiRequest<backend::screenshot_actions::Request>,
) -> ApiResponse<backend::screenshot_actions::Report> {
    let handle = app.clone();
    dispatch(resolve(&app), request, move |b, q, _| {
        backend::screenshot_actions::export(&b, q, &handle)
    })
    .await
}
command!(
    delete_screenshots,
    backend::screenshot_actions::Request,
    backend::screenshot_actions::Report,
    |b, q, _| backend::screenshot_actions::delete(&b, q)
);

command!(
    get_app_settings,
    EmptyRequest,
    backend::app_settings::Settings,
    |b, _, _| backend::app_settings::get(&b)
);
command!(
    save_app_settings,
    backend::app_settings::Settings,
    backend::app_settings::Settings,
    |b, q, _| backend::app_settings::save(&b, q)
);

command!(
    start_metadata_refresh,
    backend::metadata_refresh::Request,
    backend::metadata_refresh::Status,
    |b, q, _| backend::metadata_refresh::start(&b, q)
);
command!(
    get_metadata_refresh,
    EmptyRequest,
    backend::metadata_refresh::Status,
    |b, _, _| backend::metadata_refresh::status(&b)
);
command!(cancel_metadata_refresh, EmptyRequest, bool, |b, _, _| {
    backend::metadata_refresh::cancel(&b)
});
command!(
    get_backup_settings,
    EmptyRequest,
    backend::application_backup::ConfigView,
    |b, _, _| backend::application_backup::get_config(&b)
);
command!(
    save_backup_settings,
    backend::application_backup::SaveConfig,
    backend::application_backup::ConfigView,
    |b, q, _| backend::application_backup::save_config(&b, q)
);
command!(
    create_application_backup,
    backend::application_backup::CreateRequest,
    backend::application_backup::Backup,
    |b, q, _| backend::application_backup::create(&b, q)
);
command!(
    list_application_backups,
    EmptyRequest,
    Vec<backend::application_backup::Backup>,
    |b, _, _| backend::application_backup::list(&b)
);
command!(
    delete_application_backup,
    backend::application_backup::DeleteRequest,
    backend::application_backup::DeleteResult,
    |b, q, _| backend::application_backup::delete(&b, q)
);
command!(
    preview_application_restore,
    backend::application_backup::PreviewRequest,
    backend::application_backup::RestorePreview,
    |b, q, _| backend::application_backup::preview(&b, q)
);
command!(
    confirm_application_restore,
    backend::application_backup::ConfirmRequest,
    bool,
    |b, q, _| backend::application_backup::confirm(&b, q)
);
command!(cancel_application_restore, EmptyRequest, bool, |b, _, _| {
    backend::application_backup::cancel_restore(&b)
});
command!(
    get_application_backup_status,
    EmptyRequest,
    backend::application_backup::Status,
    |b, _, _| backend::application_backup::status(&b)
);
command!(test_webdav, EmptyRequest, bool, |b, _, _| {
    backend::webdav::test(&b)
});
command!(
    list_webdav_backups,
    EmptyRequest,
    Vec<backend::webdav::RemoteBackup>,
    |b, _, _| backend::webdav::list(&b)
);
command!(
    upload_webdav_backup,
    backend::webdav::LocalRequest,
    bool,
    |b, q, _| backend::webdav::upload(&b, q)
);
command!(
    download_webdav_backup,
    backend::webdav::RemoteRequest,
    backend::application_backup::Backup,
    |b, q, _| backend::webdav::download(&b, q)
);

command!(
    cache_remote_image,
    backend::network::ImageRequest,
    String,
    |b, q, _| backend::network::cache_image(&b, q)
);
fn login_proxy<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
    b: &backend::Backend,
    builder: tauri::WebviewWindowBuilder<'a, R, M>,
) -> backend::Result<tauri::WebviewWindowBuilder<'a, R, M>> {
    let settings = backend::app_settings::get(b)?;
    #[cfg(windows)]
    {
        use sha2::{Digest, Sha256};
        let proxy = if !settings.proxy_enabled {
            "--no-proxy-server".to_owned()
        } else if settings.proxy_mode == "manual" {
            format!(
                "--proxy-server={}",
                settings.proxy_url.replace("socks5h://", "socks5://")
            )
        } else {
            String::new()
        };
        let directory = b.data_directory.join(format!(
            "webview-auth-{:x}",
            Sha256::digest(proxy.as_bytes())
        ));
        std::fs::create_dir_all(&directory)
            .map_err(|_| backend::invalid("授权浏览器目录无法创建。"))?;
        backend::saves::files::ancestors(&directory)?;
        return Ok(builder
            .data_directory(directory)
            .additional_browser_args(&proxy));
    }
    #[cfg(not(windows))]
    {
        let _ = settings;
        Ok(builder)
    }
}
#[tauri::command]
pub async fn open_bangumi_login(
    app: tauri::AppHandle,
    request: ApiRequest<EmptyRequest>,
) -> ApiResponse<bool> {
    let handle = app.clone();
    dispatch(resolve(&app), request, move |b, _, _| {
        let url = "https://next.bgm.tv/demo/access-token"
            .parse()
            .map_err(|_| backend::invalid("授权地址无效。"))?;
        let builder = tauri::WebviewWindowBuilder::new(
            &handle,
            format!("bangumi-login-{}", backend::id()),
            tauri::WebviewUrl::External(url),
        )
        .title("登录 Bangumi")
        .inner_size(900.0, 720.0);
        login_proxy(&b, builder)?
            .build()
            .map_err(|_| backend::invalid("无法打开 Bangumi 授权窗口。"))?;
        Ok(true)
    })
    .await
}

command!(
    update_game_metadata,
    backend::detail_metadata::EditRequest,
    GameDetail,
    |b, q, _| backend::detail_metadata::edit(&b, q)
);
command!(
    set_metadata_lock,
    backend::detail_metadata::LockRequest,
    GameDetail,
    |b, q, _| backend::detail_metadata::lock(&b, q)
);

command!(
    list_game_processes,
    InstallIdRequest,
    backend::process_selection::Inventory,
    |b, q, _| backend::process_selection::list(&b, &q.install_id)
);
command!(
    select_game_process,
    backend::process_selection::SelectRequest,
    backend::process_selection::RunningProcess,
    |b, q, _| backend::process_selection::select(&b, q)
);
