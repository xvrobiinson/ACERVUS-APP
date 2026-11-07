//! StatusAvatar: the account icon. Initials in a circle, a small badge for
//! the role, and an outer ring whose color is the app's state (approved
//! 2026-11-05 with the redesign).

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ClickHandler;
use crate::ui::components::icon;
use crate::ui::theme::{Typeset as _, palette, text};

#[derive(IntoElement)]
pub struct StatusAvatar {
    id: ElementId,
    initials: SharedString,
    ring: Hsla,
    badge: IconName,
    size: f32,
    tip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl StatusAvatar {
    pub fn new(id: impl Into<ElementId>, initials: impl Into<SharedString>, ring: Hsla, badge: IconName) -> Self {
        Self { id: id.into(), initials: initials.into(), ring, badge, size: 40., tip: None, on_click: None }
    }

    pub fn tooltip(mut self, tip: impl Into<SharedString>) -> Self {
        self.tip = Some(tip.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for StatusAvatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let (size, ring) = (self.size, self.ring);
        let ring_el = div().absolute().inset_0().rounded_full().border(px(2.5)).border_color(ring);
        h_flex()
            .id(self.id)
            .relative()
            .flex_none()
            .size(px(size))
            .rounded_full()
            .child(ring_el)
            .child(
                h_flex()
                    .absolute()
                    .top(px(5.))
                    .left(px(5.))
                    .size(px(size - 10.))
                    .rounded_full()
                    .justify_center()
                    .bg(p.accent_tint)
                    .text_color(p.accent_text)
                    .typeset(text::MICRO)
                    .font_weight(FontWeight::BOLD)
                    .child(self.initials),
            )
            .child(
                h_flex()
                    .absolute()
                    .right(px(-3.))
                    .bottom(px(-3.))
                    .size(px(18.))
                    .rounded_full()
                    .justify_center()
                    .bg(p.surface)
                    .border(px(1.5))
                    .border_color(p.border)
                    .child(icon(self.badge, p.text, 10.)),
            )
            .when_some(self.tip, |d, tip| d.tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx)))
            .when_some(self.on_click, |d, f| d.cursor_pointer().on_click(move |e, w, cx| f(e, w, cx)))
    }
}
