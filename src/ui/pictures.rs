//! The owner's pictures: the streaks behind the launch splash in dark
//! (`assets/hero/`; the fog went with the sign-in screen's wash, V1
//! 2026-11-25), and WhatsApp's mark.

use std::sync::{Arc, LazyLock};

use gpui_kit::{Image, ImageFormat};

static STREAKS: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Jpeg, include_bytes!("../../assets/hero/streaks.jpg").to_vec())));

/// WhatsApp's own mark in its green (2026-11-10, the owner's pick), on
/// the buttons that open WhatsApp.
static WHATSAPP: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Svg, include_bytes!("../../assets/whatsapp.svg").to_vec())));

pub fn whatsapp() -> Arc<Image> {
    WHATSAPP.clone()
}

/// Black with light streaks, behind the splash in dark.
pub fn streaks() -> Arc<Image> {
    STREAKS.clone()
}
