use std::time::Duration;

/// The caller supplies Windows working time, which excludes sleep/hibernate.
/// Process lifetime/grace decisions use the working clock independently of idle deductions.
pub struct PlayClock {
    previous: Duration,
    counted: Duration,
    idle_threshold: Option<Duration>,
}
impl PlayClock {
    pub fn new(idle_minutes: Option<u16>) -> Self {
        Self {
            previous: Duration::ZERO,
            counted: Duration::ZERO,
            idle_threshold: idle_minutes.map(|m| Duration::from_secs(u64::from(m) * 60)),
        }
    }
    pub fn last_observed(&self) -> Duration {
        self.previous
    }
    pub fn observe(&mut self, awake: Duration, idle: Duration) -> Duration {
        let delta = awake.saturating_sub(self.previous);
        let excluded = self.idle_threshold.map_or(Duration::ZERO, |threshold| {
            idle.saturating_sub(threshold).min(delta)
        });
        self.counted += delta.saturating_sub(excluded);
        self.previous = self.previous.max(awake);
        self.counted
    }
}

#[cfg(windows)]
pub mod windows {
    use super::*;
    use crate::backend::{Result, ServiceError};
    use crate::domain::ErrorCode;
    use windows_sys::Win32::{
        System::{SystemInformation::GetTickCount, WindowsProgramming::QueryUnbiasedInterruptTime},
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    pub fn awake_time() -> Result<Duration> {
        let mut ticks = 0;
        // Valid writable pointer; Windows returns 100 ns units, excluding suspend time.
        if unsafe { QueryUnbiasedInterruptTime(&mut ticks) } == 0 {
            return Err(ServiceError(
                ErrorCode::InternalError,
                "无法读取 Windows 工作时钟，计时已停止。",
            ));
        }
        Ok(Duration::new(
            ticks / 10_000_000,
            ((ticks % 10_000_000) * 100) as u32,
        ))
    }
    pub fn idle_time() -> Result<Duration> {
        let mut input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // Reads only the last-input timestamp in this logon session, no key contents.
        if unsafe { GetLastInputInfo(&mut input) } == 0 {
            return Err(ServiceError(
                ErrorCode::InternalError,
                "无法读取 Windows 空闲状态，计时已停止。",
            ));
        }
        let age = unsafe { GetTickCount() }.wrapping_sub(input.dwTime);
        // Future/nonmonotonic SendInput ticks are treated as recent input.
        Ok(Duration::from_millis(if age > i32::MAX as u32 {
            0
        } else {
            u64::from(age)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(value: u64) -> Duration {
        Duration::from_secs(value)
    }
    #[cfg(windows)]
    #[test]
    fn windows_clock_and_idle_apis_are_available() {
        let first = super::windows::awake_time().unwrap();
        std::thread::sleep(Duration::from_millis(40));
        let second = super::windows::awake_time().unwrap();
        assert!(second >= first);
        assert!(second - first < Duration::from_secs(5));
        super::windows::idle_time().unwrap();
    }
    #[test]
    fn reading_default_counts_idle_and_suspend_does_not_advance_working_time() {
        let mut clock = PlayClock::new(None);
        assert_eq!(clock.observe(s(120), s(3600)), s(120));
        assert_eq!(clock.observe(s(120), s(7200)), s(120));
        assert_eq!(clock.observe(s(125), s(0)), s(125));
    }
    #[test]
    fn idle_deducts_only_excess_and_input_resumes_without_backfilling() {
        let mut clock = PlayClock::new(Some(1));
        assert_eq!(clock.observe(s(59), s(59)), s(59));
        assert_eq!(clock.observe(s(62), s(62)), s(60));
        assert_eq!(clock.observe(s(120), s(120)), s(60));
        assert_eq!(clock.observe(s(121), s(0)), s(61));
        assert_eq!(clock.observe(s(121), s(0)), s(61));
        assert_eq!(clock.observe(s(120), s(0)), s(61));
        assert_eq!(clock.last_observed(), s(121));
    }
}
