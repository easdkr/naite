use iced::window;

const ICON_WIDTH: u32 = 256;
const ICON_HEIGHT: u32 = 256;
const ICON_RGBA: &[u8] = include_bytes!("../assets/app-icon.rgba");

pub fn window_icon() -> Option<window::Icon> {
    window::icon::from_rgba(ICON_RGBA.to_vec(), ICON_WIDTH, ICON_HEIGHT).ok()
}

#[cfg(test)]
pub fn raw_icon_bytes() -> &'static [u8] {
    ICON_RGBA
}
