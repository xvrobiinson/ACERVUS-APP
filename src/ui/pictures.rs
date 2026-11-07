//! The owner's pictures behind the hero and the launch splash: fog in light,
//! black with light streaks in dark (`assets/hero/`).

use std::sync::{Arc, LazyLock};

use gpui_kit::{Image, ImageFormat};

static FOG: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Jpeg, include_bytes!("../../assets/hero/fog.jpg").to_vec())));
static STREAKS: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Jpeg, include_bytes!("../../assets/hero/streaks.jpg").to_vec())));

/// The picture for the theme.
pub fn hero(dark: bool) -> Arc<Image> {
    if dark { STREAKS.clone() } else { FOG.clone() }
}
