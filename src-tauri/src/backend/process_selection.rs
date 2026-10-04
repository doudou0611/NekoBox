//! A themed picker is backed by real Windows ToolHelp inventory, not file browsing.
use super::process_monitor::{Identity, Process};
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[derive(Default)]
pub struct Control {
    pub session_id: String,
    pub since: u64,
    pub baseline: HashSet<Identity>,
    pub selected: Option<Identity>,
}
pub type Controls = HashMap<String, Control>;
#[derive(Clone, Serialize)]
pub struct RunningProcess {
    pub pid: u32,
    pub created_at_ticks: String,
    pub name: String,
    pub path: String,
}
#[derive(Serialize)]
pub struct Inventory {
    pub active_session: bool,
    pub items: Vec<RunningProcess>,
}
#[derive(Deserialize)]
pub struct SelectRequest {
    pub install_id: String,
    pub pid: u32,
    pub created_at_ticks: String,
}
fn entry(process: Process) -> RunningProcess {
    RunningProcess {
        pid: process.identity.pid,
        created_at_ticks: process.identity.created.to_string(),
        name: process
            .image
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_owned(),
        path: process.image,
    }
}
fn discover() -> Result<Vec<Process>> {
    #[cfg(windows)]
    {
        process_monitor::windows::snapshot().map_err(|_| invalid("无法读取 Windows 进程列表。"))
    }
    #[cfg(not(windows))]
    {
        Err(ServiceError(
            ErrorCode::NotImplemented,
            "运行中进程选择仅支持 Windows。",
        ))
    }
}
fn allowed(process: &Process, root: &str, control: Option<&Control>) -> bool {
    process_monitor::eligible(&process.image, root, None)
        && control.is_none_or(|c| {
            process.identity.created >= c.since && !c.baseline.contains(&process.identity)
        })
}
pub fn list(b: &Backend, install_id: &str) -> Result<Inventory> {
    let installation = b.database()?.installation(install_id)?;
    let root = path_text(&absolute_directory(&installation.absolute_path)?)?;
    let processes = discover()?;
    let controls = b
        .process_controls
        .lock()
        .map_err(|_| invalid("进程跟踪服务不可用。"))?;
    let control = controls.get(install_id);
    let mut items = processes
        .into_iter()
        .filter(|p| allowed(p, &root, control))
        .map(entry)
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.name.cmp(&b.name).then(a.pid.cmp(&b.pid)));
    Ok(Inventory {
        active_session: control.is_some(),
        items,
    })
}
pub fn select(b: &Backend, q: SelectRequest) -> Result<RunningProcess> {
    let _launch = b
        .launch_lock
        .lock()
        .map_err(|_| invalid("启动服务不可用。"))?;
    let installation = b.database()?.installation(&q.install_id)?;
    let root = path_text(&absolute_directory(&installation.absolute_path)?)?;
    let identity = Identity {
        pid: q.pid,
        created: q
            .created_at_ticks
            .parse()
            .map_err(|_| invalid("进程标识无效，请刷新列表。"))?,
    };
    let process = discover()?
        .into_iter()
        .find(|p| p.identity == identity)
        .ok_or_else(|| invalid("所选进程已退出或发生变化，请刷新列表。"))?;
    let mut controls = b
        .process_controls
        .lock()
        .map_err(|_| invalid("进程跟踪服务不可用。"))?;
    let control = controls
        .get_mut(&q.install_id)
        .ok_or_else(|| invalid("当前会话已结束，所选进程可保存为下次启动配置。"))?;
    if !allowed(&process, &root, Some(control)) {
        return Err(invalid("只能选择本次启动后安装目录内的游戏进程。"));
    }
    let process = entry(process);
    b.database()?
        .record_selected_process(&control.session_id, &process.name)?;
    control.selected = Some(identity);
    Ok(process)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_scoped_to_installation_and_current_launch_identity() {
        let old = Process {
            identity: Identity {
                pid: 11,
                created: 100,
            },
            image: "C:/Games/A/game.exe".into(),
        };
        let mut c = Control {
            since: 90,
            ..Control::default()
        };
        c.baseline.insert(old.identity);
        assert!(!allowed(&old, "C:/Games/A", Some(&c)));
        let fresh = Process {
            identity: Identity {
                pid: 11,
                created: 110,
            },
            image: old.image,
        };
        assert!(allowed(&fresh, "C:/Games/A", Some(&c)));
        assert!(!allowed(&fresh, "C:/Games/AB", Some(&c)));
        assert!(!allowed(&fresh, "C:/Other", None));
    }
}
