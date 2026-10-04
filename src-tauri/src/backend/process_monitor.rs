//! Read-only process discovery. No injection, job restrictions, elevation or termination.
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Identity {
    pub pid: u32,
    pub created: u64,
}
#[derive(Debug, Clone)]
pub struct Process {
    pub identity: Identity,
    pub image: String,
}

fn path_key(path: &str) -> String {
    let path = path.replace('\\', "/").to_lowercase();
    let path = if let Some(rest) = path.strip_prefix("//?/unc/") {
        format!("//{rest}")
    } else {
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    };
    path.trim_end_matches('/').to_owned()
}
pub(crate) fn eligible(image: &str, root: &str, main: Option<&str>) -> bool {
    let image = path_key(image);
    let root = path_key(root);
    if !image
        .strip_prefix(&root)
        .is_some_and(|tail| tail.starts_with('/'))
    {
        return false;
    }
    let file = image.rsplit('/').next().unwrap_or_default();
    if let Some(main) = main {
        return file.eq_ignore_ascii_case(main);
    }
    file.ends_with(".exe")
        && ![
            "unins",
            "uninstall",
            "setup",
            "installer",
            "update",
            "updater",
            "crash",
            "vcredist",
            "dxsetup",
            "directx",
            "config",
            "settings",
            "patch",
            "galgame-manager",
            "nekobox",
            "magpie",
            "leproc",
        ]
        .iter()
        .any(|word| file.contains(word))
}
/// Policy operates on stable (PID, creation time), never on a PID/name alone.
pub struct Tracker {
    selected: Option<Identity>,
    root: String,
    main: Option<String>,
    baseline: HashSet<Identity>,
    since: u64,
    saw_game: bool,
    wait: std::time::Duration,
    launcher_is_game: bool,
    last_alive: std::time::Duration,
}
impl Tracker {
    pub fn new(root: String, main: Option<String>, baseline: &[Process], since: u64) -> Self {
        Self {
            selected: None,
            root,
            main,
            baseline: baseline.iter().map(|p| p.identity).collect(),
            since,
            saw_game: false,
            wait: std::time::Duration::from_secs(15),
            launcher_is_game: true,
            last_alive: std::time::Duration::ZERO,
        }
    }
    pub fn select(&mut self, identity: Identity) {
        self.selected = Some(identity);
    }
    pub fn external_launcher(&mut self) {
        self.launcher_is_game = false;
    }
    pub fn set_wait_seconds(&mut self, seconds: u64) {
        self.wait = std::time::Duration::from_secs(seconds.clamp(1, 300));
    }
    /// Returns true during actual activity or a bounded handoff window.
    pub fn observe(
        &mut self,
        elapsed: std::time::Duration,
        launcher_pid: u32,
        launcher_alive: bool,
        processes: &[Process],
    ) -> bool {
        if let Some(selected) = self.selected {
            if processes.iter().any(|p| p.identity == selected) {
                self.last_alive = elapsed;
                return true;
            }
            return elapsed.saturating_sub(self.last_alive) < std::time::Duration::from_secs(2);
        }
        let game_alive = processes.iter().any(|p| {
            p.identity.pid != launcher_pid
                && p.identity.created >= self.since
                && !self.baseline.contains(&p.identity)
                && eligible(&p.image, &self.root, self.main.as_deref())
        });
        // A configured main executable can also be the directly launched process.
        let direct_main = self.main.as_ref().is_some_and(|main| {
            processes.iter().any(|p| {
                p.identity.pid == launcher_pid && eligible(&p.image, &self.root, Some(main))
            })
        });
        if game_alive || direct_main {
            self.saw_game = true;
        }
        let alive = game_alive
            || direct_main
            || (launcher_alive && self.launcher_is_game && self.main.is_none());
        if alive {
            self.last_alive = elapsed;
            return true;
        }
        let grace = if self.saw_game {
            std::time::Duration::from_secs(2)
        } else {
            self.wait
        };
        elapsed.saturating_sub(self.last_alive) < grace
    }
    pub fn observed_duration(&self) -> std::time::Duration {
        self.last_alive
    }
}

#[cfg(windows)]
pub mod windows {
    use super::*;
    use std::{io, path::PathBuf};
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            SystemInformation::GetSystemTimeAsFileTime,
            Threading::{
                GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn ticks(time: FILETIME) -> u64 {
        u64::from(time.dwHighDateTime) << 32 | u64::from(time.dwLowDateTime)
    }
    pub fn creation_boundary() -> u64 {
        let mut time = FILETIME::default();
        unsafe {
            GetSystemTimeAsFileTime(&mut time);
        }
        ticks(time)
    }
    pub fn snapshot() -> io::Result<Vec<Process>> {
        // Only process inventory; never request module memory or execution rights.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = Handle(raw);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut has = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        let mut processes = Vec::new();
        while has != 0 {
            let raw =
                unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID) };
            if !raw.is_null() {
                let process = Handle(raw);
                let mut created = FILETIME::default();
                let mut exit = FILETIME::default();
                let mut kernel = FILETIME::default();
                let mut user = FILETIME::default();
                let mut image = vec![0u16; 32768];
                let mut length = image.len() as u32;
                if unsafe {
                    GetProcessTimes(process.0, &mut created, &mut exit, &mut kernel, &mut user)
                } != 0
                    && ticks(exit) == 0
                    && unsafe {
                        QueryFullProcessImageNameW(process.0, 0, image.as_mut_ptr(), &mut length)
                    } != 0
                {
                    if let Ok(image) = String::from_utf16(&image[..length as usize]) {
                        // Canonicalize path aliases / extended UNC before scoped matching.
                        let image = PathBuf::from(&image)
                            .canonicalize()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or(image);
                        processes.push(Process {
                            identity: Identity {
                                pid: entry.th32ProcessID,
                                created: ticks(created),
                            },
                            image,
                        });
                    }
                }
            }
            has = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
            return Err(io::Error::last_os_error());
        }
        Ok(processes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn p(pid: u32, created: u64, image: &str) -> Process {
        Process {
            identity: Identity { pid, created },
            image: image.into(),
        }
    }
    #[test]
    fn external_utilities_never_extend_the_configured_game_wait() {
        let mut tracker = Tracker::new("C:/Games/A".into(), None, &[], 100);
        tracker.external_launcher();
        tracker.set_wait_seconds(3);
        let tools = [
            p(12, 101, "C:/Games/A/Magpie.exe"),
            p(13, 101, "C:/Games/A/LEProc.exe"),
        ];
        assert!(tracker.observe(Duration::from_secs(2), 10, true, &tools));
        assert!(!tracker.observe(Duration::from_secs(3), 10, true, &tools));
        assert_eq!(tracker.observed_duration(), Duration::ZERO);
    }
    #[test]
    fn fast_launcher_exit_keeps_child_running_and_does_not_count_grace() {
        let mut t = Tracker::new("C:\\Games\\A".into(), None, &[], 100);
        assert!(t.observe(
            Duration::ZERO,
            10,
            false,
            &[p(11, 101, "C:\\Games\\A\\game.exe")]
        ));
        assert!(t.observe(
            Duration::from_secs(60),
            10,
            false,
            &[p(11, 101, "C:\\Games\\A\\game.exe")]
        ));
        assert!(t.observe(Duration::from_secs(61), 10, false, &[]));
        assert!(!t.observe(Duration::from_secs(62), 10, false, &[]));
        assert_eq!(t.observed_duration(), Duration::from_secs(60));
    }
    #[test]
    fn delayed_handoff_and_second_stage_are_detected_without_parent_alive() {
        let mut t = Tracker::new("C:\\Games\\A".into(), None, &[], 100);
        assert!(t.observe(Duration::from_secs(1), 10, false, &[]));
        assert!(t.observe(
            Duration::from_secs(10),
            10,
            false,
            &[p(12, 110, "C:\\Games\\A\\engine\\game.exe")]
        ));
        assert!(t.observe(Duration::from_secs(11), 10, false, &[]));
        assert!(t.observe(
            Duration::from_secs(12),
            10,
            false,
            &[p(13, 120, "C:\\Games\\A\\engine2.exe")]
        ));
        assert!(!t.observe(Duration::from_secs(14), 10, false, &[]));
        assert_eq!(t.observed_duration(), Duration::from_secs(12));
    }
    #[test]
    fn ignores_existing_old_outside_sibling_and_helper_processes() {
        let existing = p(50, 90, "C:\\Games\\A\\game.exe");
        let mut t = Tracker::new(
            "C:\\Games\\A".into(),
            None,
            std::slice::from_ref(&existing),
            100,
        );
        let noise = [
            existing,
            p(51, 99, "C:\\Games\\A\\old.exe"),
            p(52, 101, "C:\\Games\\AB\\game.exe"),
            p(53, 101, "C:\\Else\\game.exe"),
            p(54, 101, "C:\\Games\\A\\crashreport.exe"),
            p(55, 101, "C:\\Games\\A\\NekoBox.exe"),
        ];
        assert!(!t.observe(Duration::from_secs(15), 10, false, &noise));
        assert_eq!(t.observed_duration(), Duration::ZERO);
    }
    #[test]
    fn specified_main_ends_even_if_launcher_remains_and_unc_case_matches() {
        let mut t = Tracker::new(
            "\\\\?\\UNC\\Mac\\Home\\Game".into(),
            Some("GAME.exe".into()),
            &[],
            100,
        );
        assert!(t.observe(
            Duration::from_secs(1),
            10,
            true,
            &[p(11, 101, "\\\\mac\\home\\game\\game.exe")]
        ));
        assert!(!t.observe(
            Duration::from_secs(3),
            10,
            true,
            &[p(12, 102, "\\\\mac\\home\\game\\utility.exe")]
        ));
        assert_eq!(t.observed_duration(), Duration::from_secs(1));
    }
    #[test]
    fn pid_reuse_does_not_mistake_old_identity_for_new_process() {
        let old = p(11, 90, "C:\\Games\\A\\game.exe");
        let mut t = Tracker::new("C:\\Games\\A".into(), None, &[old], 100);
        assert!(t.observe(
            Duration::from_secs(5),
            10,
            false,
            &[p(11, 101, "C:\\Games\\A\\game.exe")]
        ));
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn explicit_process_selection_ends_with_that_identity_even_when_launcher_lives() {
        let identity = Identity {
            pid: 20,
            created: 110,
        };
        let mut tracker = Tracker::new("C:/Games/A".into(), None, &[], 100);
        tracker.select(identity);
        let process = Process {
            identity,
            image: "C:/Games/A/game.exe".into(),
        };
        assert!(tracker.observe(std::time::Duration::from_secs(5), 10, true, &[process]));
        let reused = Process {
            identity: Identity {
                pid: 20,
                created: 200,
            },
            image: "C:/Games/A/game.exe".into(),
        };
        assert!(!tracker.observe(std::time::Duration::from_secs(7), 10, true, &[reused]));
        assert_eq!(
            tracker.observed_duration(),
            std::time::Duration::from_secs(5)
        );
    }
}
