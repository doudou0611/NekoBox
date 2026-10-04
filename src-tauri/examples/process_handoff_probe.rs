//! Synthetic Windows regression probe. Never launches an installed game or opens app data.
#[cfg(windows)]
#[path = "../src/backend/process_monitor.rs"]
mod process_monitor;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    let exe = std::env::current_exe()?;
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "worker" {
        std::thread::sleep(Duration::from_secs(6));
        return Ok(());
    }
    if mode == "launcher" {
        let mut args = std::env::args().skip(2);
        let worker = args.next().ok_or("missing worker path")?;
        let delay = args.next().unwrap_or_default().parse::<u64>()?;
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        // Launcher exits immediately after starting the longer-lived synthetic game.
        Command::new(worker).arg("worker").spawn()?;
        return Ok(());
    }
    let root = std::env::temp_dir().join(format!("galgame-handoff-probe-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    let launcher = root.join("launcher.exe");
    let worker = root.join("game.exe");
    std::fs::copy(&exe, &launcher)?;
    std::fs::copy(&exe, &worker)?;
    let root = root.canonicalize()?;
    for (main, delay) in [(None, 0), (Some("game.exe".to_owned()), 800)] {
        let baseline = process_monitor::windows::snapshot()?;
        let boundary = process_monitor::windows::creation_boundary();
        let mut tracker = process_monitor::Tracker::new(
            root.to_string_lossy().into_owned(),
            main.clone(),
            &baseline,
            boundary,
        );
        let start = Instant::now();
        let mut child = Command::new(&launcher)
            .arg("launcher")
            .arg(&worker)
            .arg(delay.to_string())
            .spawn()?;
        let mut exit = None;
        let mut alive_after_exit = false;
        loop {
            if exit.is_none() {
                exit = child.try_wait()?;
            }
            let elapsed = start.elapsed();
            let processes = process_monitor::windows::snapshot()?;
            if !tracker.observe(elapsed, child.id(), exit.is_none(), &processes) {
                break;
            }
            if elapsed > Duration::from_secs(2) && exit.is_some() {
                alive_after_exit = true;
            }
            if elapsed > Duration::from_secs(20) {
                return Err("monitor never ended".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let observed = tracker.observed_duration().as_secs_f64();
        assert!(alive_after_exit, "session ended with launcher");
        assert!(
            (5.5..8.0).contains(&observed),
            "bad observed duration {observed}"
        );
        assert!(
            start.elapsed().as_secs_f64() - observed < 3.0,
            "grace counted or excessive ending delay"
        );
        println!(
            "PASS main={main:?} launcher_exited=true observed={observed:.2}s total={:.2}s",
            start.elapsed().as_secs_f64()
        );
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    println!("Windows-only synthetic handoff probe; no game executed.");
}
