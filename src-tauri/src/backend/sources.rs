use super::{Backend, Result};
use crate::{
    backend::types::{BindSourceRequest, ExternalSourcesRequest, OpenSourceRequest},
    domain::models::ExternalSource,
};

impl Backend {
    pub fn bind_external_source(&self, request: BindSourceRequest) -> Result<Vec<ExternalSource>> {
        self.database()?.bind_external_source(&request)
    }
    pub fn open_external_source(&self, request: OpenSourceRequest) -> Result<bool> {
        let sources = self.database()?.list_external_sources(&request.game_id)?;
        if !sources.iter().any(|source| {
            source.url == request.url
                || super::hikarinagi_rates::allowed_rating_url(source, &request.url)
        }) {
            return Err(super::invalid("只能打开该作品已绑定的官方来源。"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::UI::Shell::ShellExecuteW;
            let url: Vec<u16> = std::ffi::OsStr::new(&request.url)
                .encode_wide()
                .chain(Some(0))
                .collect();
            let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
            let result = unsafe {
                ShellExecuteW(
                    std::ptr::null_mut(),
                    verb.as_ptr(),
                    url.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                )
            };
            if result as isize <= 32 {
                return Err(super::invalid("系统浏览器无法打开链接。"));
            }
        }
        #[cfg(target_os = "macos")]
        if !std::process::Command::new("/usr/bin/open")
            .arg(&request.url)
            .status()
            .map_err(|_| super::invalid("无法打开系统浏览器。"))?
            .success()
        {
            return Err(super::invalid("无法打开系统浏览器。"));
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        return Err(super::invalid("当前平台尚未接入外部浏览器。"));
        #[allow(unreachable_code)]
        Ok(true)
    }
    pub fn list_external_sources(
        &self,
        request: ExternalSourcesRequest,
    ) -> Result<Vec<ExternalSource>> {
        self.database()?.list_external_sources(&request.game_id)
    }
}
