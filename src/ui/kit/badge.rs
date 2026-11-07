//! Badge: a small count on the top right corner of a nav tab's icon or the
//! bell (picked 2026-11-07), colored by what it is about: red when
//! something is late (overdue loans), amber when someone is waiting on you
//! (collector payments to confirm), blue for news (new messages). Counts
//! over 99 read "99+". The caller places it on the icon's corner.

use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::ui::theme::{Typeset as _, palette, tabular, text};

/// What the count is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
    /// Something is late.
    Alert,
    /// Someone is waiting on you.
    Waiting,
    /// News.
    Info,
}

#[derive(IntoElement)]
pub struct Badge {
    count: usize,
    tone: BadgeTone,
}

impl Badge {
    pub fn new(count: usize, tone: BadgeTone) -> Self {
        Self { count, tone }
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let b = palette(cx).badge();
        let fill = match self.tone {
            BadgeTone::Alert => b.alert,
            BadgeTone::Waiting => b.waiting,
            BadgeTone::Info => b.info,
        };
        let shown = if self.count > 99 { "99+".to_string() } else { self.count.to_string() };
        h_flex()
            .flex_none()
            .h(px(15.))
            .min_w(px(15.))
            .px(px(4.))
            .justify_center()
            .rounded_full()
            .bg(fill)
            .typeset(text::MICRO)
            .text_size(px(10.))
            .line_height(px(15.))
            .font_weight(FontWeight::SEMIBOLD)
            .font_features(tabular())
            .text_color(b.on)
            .child(shown)
    }
}
