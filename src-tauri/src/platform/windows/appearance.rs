//! System appearance: app theme (registry), accent (DWM) and animation preference.

use serde::Serialize;
use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};

use super::registry;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAppearance {
    pub dark: Option<bool>,
    pub accent: Option<String>,
    pub reduce_motion: Option<bool>,
}

const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

pub fn read() -> SystemAppearance {
    let dark = registry::read_dword(HKEY_CURRENT_USER, PERSONALIZE, "AppsUseLightTheme").map(|light| light == 0);
    let mut color = 0u32;
    let mut opaque = windows::core::BOOL::default();
    let accent = unsafe { DwmGetColorizationColor(&mut color, &mut opaque) }
        .ok()
        .map(|_| format!("#{:06x}", color & 0x00ff_ffff));
    let mut animations = windows::core::BOOL::default();
    let reduce_motion = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut animations as *mut windows::core::BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()
    .map(|_| !animations.as_bool());
    SystemAppearance { dark, accent, reduce_motion }
}
