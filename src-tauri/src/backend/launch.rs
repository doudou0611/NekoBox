use super::{types::*, *};
use crate::domain::{models::LaunchSession, requests::LaunchGameRequest};
use std::path::Path;

fn external_tool(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        || path
            .ancestors()
            .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| scanner::linked(&m)))
    {
        return Err(invalid("请在设置中选择外部工具的完整 EXE 路径。"));
    }
    let path = path
        .canonicalize()
        .map_err(|_| invalid("外部工具不存在，请重新配置。"))?;
    if !path.is_file() {
        return Err(invalid("外部工具不是可执行文件。"));
    }
    pe::inspect(&path)?;
    Ok(path)
}

fn executable(root: &Path, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute()
        || path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("exe") && !e.eq_ignore_ascii_case("bat"))
        || scanner::ignored_executable(path)
    {
        return Err(ServiceError(
            ErrorCode::PathInvalid,
            "请选择游戏目录内的 exe 或 bat；安装器、更新器和卸载器不可作为启动入口。",
        ));
    }
    if path
        .ancestors()
        .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| scanner::linked(&m)))
    {
        return Err(ServiceError(
            ErrorCode::PathInvalid,
            "启动入口不能经过符号链接。",
        ));
    }
    let path = path
        .canonicalize()
        .map_err(|_| ServiceError(ErrorCode::PathInvalid, "启动文件不存在或不可访问。"))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(ServiceError(
            ErrorCode::PathInvalid,
            "启动入口必须位于游戏安装目录内。",
        ));
    }
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        pe::inspect(&path)?;
    }
    Ok(path)
}
fn options(r: &ConfigureInstallationRequest) -> Result<()> {
    if r.idle_timeout_minutes
        .is_some_and(|minutes| !(1..=120).contains(&minutes))
    {
        return Err(invalid("空闲暂停阈值必须在 1～120 分钟之间。"));
    }
    if r.main_process_name.as_ref().is_some_and(|name| {
        name.len() > 256
            || name.contains(['/', '\\', '\0', ':', '*', '?', '"', '<', '>', '|'])
            || !name.to_lowercase().ends_with(".exe")
            || name.trim() != name
            || name.len() <= 4
    }) {
        return Err(invalid(
            "主进程名请填写 exe 文件名，例如 game.exe；不能包含路径或通配符。",
        ));
    }
    if r.arguments.len() > 128
        || r.arguments
            .iter()
            .any(|s| s.len() > 4096 || s.contains('\0'))
        || r.arguments.iter().map(String::len).sum::<usize>() > 65536
        || r.environment.len() > 64
        || r.environment.iter().any(|(k, v)| {
            k.is_empty()
                || k.len() > 128
                || k.contains(['=', '\0'])
                || v.len() > 4096
                || v.contains('\0')
        })
    {
        return Err(invalid("启动参数或环境变量超过限制，或包含无效字符。"));
    }
    Ok(())
}
#[cfg(windows)]
fn steam_executable() -> Result<PathBuf> {
    external_tool(&path_text(&steam::scan::install_path()?.join("steam.exe"))?)
}

impl Backend {
    pub fn configure_installation(
        &self,
        r: &ConfigureInstallationRequest,
    ) -> Result<InstallationDetails> {
        options(r)?;
        let installation = self.database()?.installation(&r.install_id)?;
        let root = absolute_directory(&installation.absolute_path)?;
        let steam_launch = matches!(
            installation.source,
            crate::domain::protocol::InstallSource::Steam
        );
        if steam_launch && r.steam_app_id != installation.steam_app_id {
            return Err(invalid("Steam 安装的 AppID 由清单确定，不能修改。"));
        }
        let entry = if steam_launch && r.executable_path.is_empty() {
            String::new()
        } else {
            path_text(&executable(&root, &r.executable_path)?)?
        };
        let working_directory = r
            .working_directory
            .as_ref()
            .map(|p| absolute_directory(p).and_then(|p| path_text(&p)))
            .transpose()?;
        let normalized = ConfigureInstallationRequest {
            install_id: r.install_id.clone(),
            executable_path: entry,
            steam_app_id: r.steam_app_id.clone(),
            arguments: r.arguments.clone(),
            environment: r.environment.clone(),
            working_directory,
            main_process_name: r.main_process_name.clone(),
            track_after_launcher_exit: steam_launch || r.track_after_launcher_exit,
            idle_timeout_minutes: r.idle_timeout_minutes,
            use_locale_emulator: if steam_launch {
                Some(false)
            } else {
                r.use_locale_emulator
            },
            use_magpie: r.use_magpie,
        };
        self.database()?.save_installation(&normalized)
    }
    pub fn launch(
        &self,
        r: &LaunchGameRequest,
        request_id: &str,
        app: Option<tauri::AppHandle>,
    ) -> Result<LaunchSession> {
        if !r.options.user_initiated {
            return Err(invalid("必须由用户主动启动。"));
        }
        let _guard = self
            .launch_lock
            .lock()
            .map_err(|_| ServiceError(ErrorCode::InternalError, "启动服务需要重新启动。"))?;
        let installation = self.database()?.installation(&r.install_id)?;
        if self
            .hikarifield_downloads
            .lock()
            .map_err(|_| invalid("下载服务需要重新启动。"))?
            .tasks
            .iter()
            .any(|t| {
                t.game_id == installation.game_id
                    && matches!(t.status.as_str(), "queued" | "running")
            })
        {
            return Err(invalid("请等待 HIKARI FIELD 下载完成，再启动游戏。"));
        }
        let root = absolute_directory(&installation.absolute_path)?;
        let steam_launch = matches!(
            installation.source,
            crate::domain::protocol::InstallSource::Steam
        ) && installation
            .steam_app_id
            .as_deref()
            .is_some_and(|id| steam::scan::app_id(id).is_ok());
        if steam_launch {
            steam::scan::verify(
                &installation.absolute_path,
                installation.steam_app_id.as_deref().unwrap(),
            )?;
        }
        let exe = if steam_launch {
            #[cfg(windows)]
            {
                steam_executable()?
            }
            #[cfg(not(windows))]
            {
                return Err(ServiceError(
                    ErrorCode::NotImplemented,
                    "Steam 启动请在 Windows 使用。",
                ));
            }
        } else {
            let value = installation.executable_path.as_ref().ok_or(ServiceError(
                ErrorCode::Conflict,
                "请先在“启动”页选择并保存启动入口。",
            ))?;
            executable(&root, value)?
        };
        let mut config = ConfigureInstallationRequest {
            install_id: r.install_id.clone(),
            executable_path: path_text(&exe)?,
            steam_app_id: installation.steam_app_id.clone(),
            arguments: installation.arguments,
            working_directory: installation.working_directory,
            environment: installation.environment,
            main_process_name: installation.main_process_name,
            track_after_launcher_exit: installation.track_after_launcher_exit,
            idle_timeout_minutes: installation.idle_timeout_minutes,
            use_locale_emulator: installation.use_locale_emulator,
            use_magpie: installation.use_magpie,
        };
        if steam_launch {
            let id = installation.steam_app_id.clone().unwrap_or_default();
            let mut arguments = vec!["-silent".into()];
            if config.arguments.is_empty() {
                arguments.push(format!("steam://rungameid/{id}"));
            } else {
                arguments.extend(["-applaunch".into(), id]);
                arguments.append(&mut config.arguments);
            }
            config.arguments = arguments;
            config.use_locale_emulator = Some(false);
            config.working_directory = Some(root.to_string_lossy().into_owned());
            config.track_after_launcher_exit = true;
        }
        options(&config)?;
        let preferences = app_settings::get(self)?;
        let use_le = config
            .use_locale_emulator
            .unwrap_or(preferences.default_locale_emulator);
        let use_magpie = config.use_magpie.unwrap_or(preferences.default_magpie);
        let le = use_le
            .then(|| external_tool(&preferences.locale_emulator_path))
            .transpose()?;
        let magpie = use_magpie
            .then(|| external_tool(&preferences.magpie_path))
            .transpose()?;
        if use_le
            && exe
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("bat"))
        {
            return Err(invalid("Locale Emulator 仅支持 EXE 游戏入口。"));
        }
        let work = config
            .working_directory
            .as_ref()
            .map(|p| absolute_directory(p))
            .transpose()?
            .unwrap_or(root.clone());
        if self.database()?.has_open_session(&installation.game_id)? {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "此作品已经有运行中的会话。",
            ));
        }
        #[cfg(not(windows))]
        {
            let _ = (config, work, request_id, app, preferences, le, magpie);
            Err(ServiceError(
                ErrorCode::NotImplemented,
                "当前平台仅支持扫描和管理；游戏启动请在 Windows x64 测试。",
            ))
        }
        #[cfg(windows)]
        {
            use tauri::Emitter;
            self.automatic_save_backup(&r.install_id, true, request_id, app.as_ref())?;
            // Snapshot before spawning: never attach to a pre-existing game or an unrelated PID.
            let baseline = process_monitor::windows::snapshot().map_err(|_| {
                ServiceError(
                    ErrorCode::InternalError,
                    "无法读取 Windows 进程列表，未启动游戏。",
                )
            })?;
            let boundary = process_monitor::windows::creation_boundary();
            let mut tracker = process_monitor::Tracker::new(
                path_text(&root)?,
                config.main_process_name.clone(),
                &baseline,
                boundary,
            );
            if steam_launch {
                tracker.external_launcher();
            }
            tracker.set_wait_seconds(preferences.launch_wait_seconds);
            if le.is_some() {
                tracker.external_launcher();
            }
            if let Some(path) = magpie {
                let running = baseline.iter().any(|p| {
                    Path::new(&p.image)
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("Magpie.exe"))
                });
                if !running {
                    std::process::Command::new(&path)
                        .arg("-t")
                        .current_dir(path.parent().unwrap_or(&root))
                        .spawn()
                        .map_err(|_| {
                            ServiceError(
                                ErrorCode::PermissionDenied,
                                "Magpie 启动失败，未启动游戏。",
                            )
                        })?;
                }
            }
            let start = play_clock::windows::awake_time()?;
            let mut clock = play_clock::PlayClock::new(config.idle_timeout_minutes);
            let session = LaunchSession {
                session_id: id(),
                game_id: installation.game_id,
                install_id: r.install_id.clone(),
                started_at: now(),
            };
            let process = exe.file_name().unwrap_or_default().to_string_lossy();
            self.database()?.begin_session(&session, &process)?;
            // EXE files use the direct process API. Batch files are launched
            // through the Windows command interpreter because the OS cannot
            // create a process from a .bat path directly.
            let mut command = if let Some(le) = le {
                let mut command = std::process::Command::new(le);
                command.arg(&exe);
                command
            } else if exe
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("bat"))
            {
                let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
                let mut command = std::process::Command::new(shell);
                command.args(["/D", "/C"]).arg(&exe);
                command
            } else {
                std::process::Command::new(&exe)
            };
            let spawned = command
                .args(&config.arguments)
                .envs(&config.environment)
                .current_dir(work)
                .spawn();
            let mut child = match spawned {
                Ok(child) => child,
                Err(_) => {
                    self.database()?.end_session(
                        &session.session_id,
                        &now(),
                        0,
                        "launch_failed",
                    )?;
                    return Err(ServiceError(
                        ErrorCode::PermissionDenied,
                        "Windows 未能启动文件，请检查入口、权限或依赖。",
                    ));
                }
            };
            self.process_controls
                .lock()
                .map_err(|_| invalid("进程跟踪服务不可用。"))?
                .insert(
                    r.install_id.clone(),
                    process_selection::Control {
                        session_id: session.session_id.clone(),
                        since: boundary,
                        baseline: baseline.iter().map(|p| p.identity).collect(),
                        selected: None,
                    },
                );
            if let Some(app) = &app {
                let _ = app.emit(
                    "playtime:session-started",
                    crate::domain::EventEnvelope {
                        request_id: request_id.to_owned(),
                        occurred_at: now(),
                        payload: &session,
                    },
                );
            }
            let backend = self.clone();
            let request_id = request_id.to_owned();
            let returned = LaunchSession {
                session_id: session.session_id.clone(),
                game_id: session.game_id.clone(),
                install_id: session.install_id.clone(),
                started_at: session.started_at.clone(),
            };
            std::thread::spawn(move || {
                let mut last_checkpoint = 0;
                let mut launcher_exit = None;
                let mut monitor_errors = 0;
                let mut duration_seconds = 0;
                let mut ended_at = session.started_at.clone();
                let end_reason = loop {
                    if launcher_exit.is_none() {
                        match child.try_wait() {
                            Ok(Some(status)) => launcher_exit = Some(status),
                            Err(_) => break "monitor_error",
                            Ok(None) => {}
                        }
                    }
                    let elapsed = match play_clock::windows::awake_time() {
                        Ok(now) => now.saturating_sub(start),
                        Err(_) => break "monitor_error",
                    };
                    let idle = if config.idle_timeout_minutes.is_some() {
                        match play_clock::windows::idle_time() {
                            Ok(idle) => idle,
                            Err(_) => break "monitor_error",
                        }
                    } else {
                        std::time::Duration::ZERO
                    };
                    let selected_process = backend
                        .process_controls
                        .lock()
                        .ok()
                        .and_then(|c| c.get(&session.install_id).and_then(|c| c.selected));
                    if let Some(identity) = selected_process {
                        tracker.select(identity);
                    }
                    if !config.track_after_launcher_exit && selected_process.is_none() {
                        duration_seconds = clock.observe(elapsed, idle).as_secs();
                        ended_at = now();
                        if let Some(status) = launcher_exit {
                            break if status.success() {
                                "process_exit"
                            } else {
                                "process_error"
                            };
                        }
                    } else {
                        match process_monitor::windows::snapshot() {
                            Ok(processes) => {
                                monitor_errors = 0;
                                let running = tracker.observe(
                                    elapsed,
                                    child.id(),
                                    launcher_exit.is_none(),
                                    &processes,
                                );
                                let observed = tracker.observed_duration();
                                if observed > clock.last_observed() {
                                    duration_seconds = clock.observe(observed, idle).as_secs();
                                    ended_at = now();
                                }
                                if !running {
                                    break "tracked_process_exit";
                                }
                            }
                            Err(_) => {
                                monitor_errors += 1;
                                if monitor_errors >= 3 {
                                    break "monitor_error";
                                }
                            }
                        }
                    }
                    if let Ok(mut live) = backend.active_playtime.lock() {
                        live.insert(session.session_id.clone(), duration_seconds);
                    }
                    if duration_seconds >= last_checkpoint + 15 {
                        let persisted = backend.database().and_then(|mut db| {
                            db.checkpoint_session(&session.session_id, &ended_at, duration_seconds)
                        });
                        if persisted.is_err() {
                            eprintln!("计时检查点保存失败，异常恢复仅保留此前已保存时间。");
                        }
                        last_checkpoint = duration_seconds;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                };
                let _launch = match backend.launch_lock.lock() {
                    Ok(guard) => guard,
                    Err(_) => return,
                };
                if let Ok(mut controls) = backend.process_controls.lock() {
                    controls.remove(&session.install_id);
                }
                let persisted = backend.database().and_then(|db| {
                    db.end_session(&session.session_id, &ended_at, duration_seconds, end_reason)
                });
                if persisted.is_err() {
                    eprintln!("会话结束记录写入失败，下次启动将标记为中断。");
                    return;
                }
                if let Ok(mut live) = backend.active_playtime.lock() {
                    live.remove(&session.session_id);
                }
                let save_backup_error = if end_reason == "monitor_error" {
                    match backend.skipped_exit_save_backup(&session.install_id) {
                        Ok(message) => message,
                        Err(error) => Some(error.1.to_owned()),
                    }
                } else {
                    backend
                        .automatic_save_backup(
                            &session.install_id,
                            false,
                            &request_id,
                            app.as_ref(),
                        )
                        .err()
                        .map(|e| e.1.to_owned())
                };
                application_backup::trigger(&backend, "game_exit");
                #[derive(Clone, serde::Serialize)]
                struct Ended {
                    #[serde(flatten)]
                    session: LaunchSession,
                    ended_at: String,
                    duration_seconds: u64,
                    end_reason: &'static str,
                    save_backup_error: Option<String>,
                }
                if let Some(app) = app {
                    let _ = app.emit(
                        "playtime:session-ended",
                        crate::domain::EventEnvelope {
                            request_id,
                            occurred_at: ended_at.clone(),
                            payload: Ended {
                                session,
                                ended_at,
                                duration_seconds,
                                end_reason,
                                save_backup_error,
                            },
                        },
                    );
                }
            });
            Ok(returned)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_nul_and_invalid_environment_keys() {
        let mut r = ConfigureInstallationRequest {
            install_id: "test".into(),
            executable_path: "/x.exe".into(),
            arguments: vec!["--name=hello world".into()],
            working_directory: None,
            environment: Default::default(),
            steam_app_id: None,
            main_process_name: None,
            track_after_launcher_exit: true,
            idle_timeout_minutes: None,
            use_locale_emulator: None,
            use_magpie: None,
        };
        assert!(options(&r).is_ok());
        r.arguments.push("bad\0arg".into());
        assert!(options(&r).is_err());
        r.arguments.clear();
        r.environment.insert("A=B".into(), "x".into());
        assert!(options(&r).is_err());
    }
    #[test]
    fn main_process_is_a_plain_exe_name_and_old_payload_defaults_to_handoff() {
        let payload = serde_json::json!({"install_id":"fixture","executable_path":"C:\\Games\\A\\launcher.exe","arguments":[],"working_directory":null,"environment":{}});
        let mut r: ConfigureInstallationRequest = serde_json::from_value(payload).unwrap();
        assert!(r.track_after_launcher_exit);
        assert!(r.main_process_name.is_none());
        assert!(r.idle_timeout_minutes.is_none());
        for name in [
            "../game.exe",
            "C:\\game.exe",
            "*.exe",
            "game",
            ".exe",
            " game.exe",
            "game.exe ",
        ] {
            r.main_process_name = Some(name.into());
            assert!(options(&r).is_err(), "{name}");
        }
        r.main_process_name = Some("游戏.exe".into());
        assert!(options(&r).is_ok());
    }
}
