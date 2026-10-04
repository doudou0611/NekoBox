//! Native window backdrop, not a CSS blur of application content.
//! The transparent webview exposes the OS material beneath the rail and main content.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowMaterial {
    Solid,
    #[cfg(any(target_os = "macos", test))]
    MacosVibrancy,
    #[cfg(any(target_os = "windows", test))]
    WindowsAcrylic,
    #[cfg(any(target_os = "windows", test))]
    WindowsVirtualMachine,
}

impl WindowMaterial {
    pub(crate) fn script(self) -> &'static str {
        match self {
            Self::Solid => "document.documentElement.dataset.windowMaterial = 'solid';",
            #[cfg(any(target_os = "macos", test))]
            Self::MacosVibrancy => {
                "document.documentElement.dataset.windowMaterial = 'macos-vibrancy';"
            }
            #[cfg(any(target_os = "windows", test))]
            Self::WindowsAcrylic => {
                "document.documentElement.dataset.windowMaterial = 'windows-acrylic';"
            }
            #[cfg(any(target_os = "windows", test))]
            Self::WindowsVirtualMachine => {
                "document.documentElement.dataset.windowMaterial = 'windows-virtual-machine'; document.documentElement.dataset.renderProfile = 'virtual-machine';"
            }
        }
    }
}

pub(crate) fn apply(window: &tauri::WebviewWindow) -> WindowMaterial {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        if apply_vibrancy(
            window,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::Active),
            None,
        )
        .is_ok()
        {
            return WindowMaterial::MacosVibrancy;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if virtual_machine_profile(std::env::var("NEKOBOX_RENDER_PROFILE").ok().as_deref()) {
            eprintln!("已启用虚拟机渲染档：保留交互动效，停用全窗 Acrylic。");
            return WindowMaterial::WindowsVirtualMachine;
        }
        // Mica samples a wallpaper approximation, not the actual windows behind us.
        if window_vibrancy::apply_acrylic(window, Some((0, 0, 0, 1))).is_ok() {
            return WindowMaterial::WindowsAcrylic;
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = window;
    eprintln!("原生桌面材质不可用，已回退为实色界面。");
    WindowMaterial::Solid
}

#[cfg(any(target_os = "windows", test))]
fn virtual_machine_profile(override_value: Option<&str>) -> bool {
    override_value == Some("virtual-machine")
}

#[cfg(target_os = "windows")]
pub(crate) fn configure<R: tauri::Runtime>(mut context: tauri::Context<R>) -> tauri::Context<R> {
    let manufacturer = windows_manufacturer();
    let external = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
    for window in &mut context.config_mut().app.windows {
        if window.label == "main" {
            window.additional_browser_args = rasterization_args(
                window.additional_browser_args.as_deref(),
                &external,
                &manufacturer,
            );
        }
    }
    context
}

#[cfg(any(target_os = "windows", test))]
fn rasterization_args(
    existing: Option<&str>,
    external: &str,
    manufacturer: &str,
) -> Option<String> {
    let disabled = existing
        .unwrap_or_default()
        .split_whitespace()
        .chain(external.split_whitespace())
        .any(|arg| {
            matches!(
                arg.split('=').next(),
                Some("--disable-gpu" | "--disable-gpu-rasterization")
            )
        });
    if !manufacturer.to_ascii_lowercase().contains("parallels") || disabled {
        return existing.map(str::to_owned);
    }
    let args = existing.unwrap_or_default();
    if args
        .split_whitespace()
        .chain(external.split_whitespace())
        .any(|arg| arg == "--enable-gpu-rasterization")
    {
        return existing.map(str::to_owned);
    }
    Some(
        format!("{args} --enable-gpu-rasterization")
            .trim()
            .to_owned(),
    )
}

#[cfg(target_os = "windows")]
fn windows_manufacturer() -> String {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let key: Vec<u16> = "HARDWARE\\DESCRIPTION\\System\\BIOS\0"
        .encode_utf16()
        .collect();
    let name: Vec<u16> = "SystemManufacturer\0".encode_utf16().collect();
    let mut value = [0u16; 256];
    let mut size = std::mem::size_of_val(&value) as u32;
    // Fixed-size local buffer, read-only predefined key; no handle to close.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            value.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return String::new();
    }
    let length = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length])
}

#[cfg(test)]
mod tests {
    use super::{rasterization_args, virtual_machine_profile, WindowMaterial};

    #[test]
    fn full_visuals_are_default_and_solid_fallback_is_explicit() {
        assert!(!virtual_machine_profile(None));
        assert!(!virtual_machine_profile(Some("full")));
        assert!(virtual_machine_profile(Some("virtual-machine")));
        assert!(!virtual_machine_profile(Some("unknown")));
    }

    #[test]
    fn rasterization_is_scoped_and_preserves_user_arguments() {
        assert_eq!(
            rasterization_args(None, "", "Parallels"),
            Some("--enable-gpu-rasterization".into())
        );
        assert_eq!(rasterization_args(None, "", "Dell"), None);
        assert_eq!(rasterization_args(None, "--disable-gpu", "Parallels"), None);
        assert_eq!(
            rasterization_args(Some("--disable-gpu-rasterization"), "", "Parallels"),
            Some("--disable-gpu-rasterization".into())
        );
        assert_eq!(
            rasterization_args(
                Some("--proxy-server=http://localhost:1234"),
                "",
                "Parallels"
            ),
            Some("--proxy-server=http://localhost:1234 --enable-gpu-rasterization".into())
        );
        assert_eq!(
            rasterization_args(Some("--enable-gpu-rasterization"), "", "Parallels"),
            Some("--enable-gpu-rasterization".into())
        );
        assert_eq!(
            rasterization_args(None, "--enable-gpu-rasterization", "Parallels"),
            None
        );
    }

    #[test]
    fn vm_marker_preserves_full_motion_policy() {
        let script = WindowMaterial::WindowsVirtualMachine.script();
        assert!(script.contains("windows-virtual-machine"));
        assert!(script.contains("renderProfile = 'virtual-machine'"));
        assert!(!script.contains("dataset.motion"));
    }

    #[test]
    fn material_scripts_are_static_and_disjoint() {
        let scripts = [
            WindowMaterial::Solid,
            WindowMaterial::MacosVibrancy,
            WindowMaterial::WindowsAcrylic,
        ]
        .map(WindowMaterial::script);
        assert_ne!(scripts[0], scripts[1]);
        assert_ne!(scripts[1], scripts[2]);
        for script in scripts {
            assert!(script.starts_with("document.documentElement.dataset.windowMaterial = '"));
            assert!(script.ends_with("';"));
        }
    }
}
