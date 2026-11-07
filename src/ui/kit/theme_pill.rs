//! ThemePill: light or dark in one click, a sun | moon pill whose thumb
//! glides to the theme in use (approved 2026-11-05 with the redesign).

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::*;

use super::ClickHandler;
use super::interact::scoped;
use crate::ui::components::icon;
use crate::ui::theme::{Elevation, elevation, motion, palette};

const CELL: f32 = 30.;

#[derive(IntoElement)]
pub struct ThemePill {
    id: ElementId,
    dark: bool,
    tip: SharedString,
    on_toggle: ClickHandler,
}

impl ThemePill {
    /// `dark`: the theme in use. `tip` says what a click does.
    pub fn new(
        id: impl Into<ElementId>,
        dark: bool,
        tip: impl Into<SharedString>,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), dark, tip: tip.into(), on_toggle: std::rc::Rc::new(on_toggle) }
    }
}

impl RenderOnce for ThemePill {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let (dark, f, tip) = (self.dark, self.on_toggle, self.tip);
        let cell = |name: IconName, on: bool| {
            h_flex().relative().size(px(CELL)).h(px(26.)).justify_center().child(icon(name, if on { p.text } else { p.faint }, 14.))
        };
        let thumb = div()
            .absolute()
            .top(px(3.))
            .w(px(CELL))
            .h(px(26.))
            .rounded_full()
            .bg(if p.dark { p.active } else { p.surface })
            .shadow(elevation(&p, Elevation::Raised))
            .with_spring(
                scoped(&self.id, "thumb"),
                SpringAnimation::new(motion::GLIDE).to(px(if dark { 3. + CELL } else { 3. })),
                |d, x| d.left(x),
            );
        h_flex()
            .id(self.id)
            .relative()
            .flex_none()
            .p(px(3.))
            .rounded_full()
            .border_1()
            .border_color(p.border)
            .bg(p.hover)
            .cursor_pointer()
            .child(thumb)
            .child(cell(IconName::Sun, !dark))
            .child(cell(IconName::Moon, dark))
            .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
            .on_click(move |e, w, cx| f(e, w, cx))
    }
}
