//! Chip: a small rounded label with an icon that says how something stands
//! (the cloud sync) and opens where it can be looked at. Neutral at rest;
//! `.accent()` when it needs the person. Never a money color: those describe
//! money only, except `.done()`: a brief green "all done" after work the
//! chip was showing (the sync). `.spinning()` turns the icon while that
//! work runs. Clickable chips fade a fill in under the pointer.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ClickHandler;
use super::interact::{hover_state, mix, scoped};
use crate::ui::components::icon;
use crate::ui::theme::{RADIUS_CHIP, Typeset as _, motion, palette, space, tabular, text};

#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    accent: bool,
    spinning: bool,
    done: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, accent: false, spinning: false, done: false, tooltip: None, on_click: None }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Ledger Blue: something here needs the person.
    pub fn accent(mut self, accent: bool) -> Self {
        self.accent = accent;
        self
    }

    /// The icon turns: work is under way.
    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    /// Green for a moment: the work just finished. The icon pops in.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    pub fn tooltip(mut self, tip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tip.into());
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let (hovered, on_hover) = hover_state(&self.id, window, cx);
        let (rest, under, fg) = if self.done {
            (p.positive_tint, p.positive_tint, p.positive)
        } else if self.accent {
            (p.accent_tint, p.accent_tint, p.accent_text)
        } else {
            (p.neutral_tint, p.hover, p.muted)
        };
        let (id, spinning, done) = (self.id.clone(), self.spinning, self.done);
        let mark = self.icon.map(|i| {
            let el = icon(i, fg, 14.);
            if spinning {
                el.with_animation(
                    scoped(&id, "spin"),
                    Animation::new(std::time::Duration::from_millis(900)).repeat().with_easing(ease_in_out),
                    |el, t| el.transform(Transformation::rotate(percentage(t))),
                )
                .into_any_element()
            } else if done {
                el.with_animation(scoped(&id, "done"), Animation::new(std::time::Duration::from_millis(320)), |el, t| {
                    el.size(px(14. * (0.4 + 0.6 * super::motion::ease_out(t))))
                })
                .into_any_element()
            } else {
                el.into_any_element()
            }
        });
        let clickable = self.on_click.is_some();
        let accent = self.accent;
        let tooltip = self.tooltip.clone();
        h_flex()
            .id(self.id.clone())
            .flex_none()
            .h(px(26.))
            .px(px(space::S + 2.))
            .gap(px(6.))
            .rounded(px(RADIUS_CHIP))
            .text_color(fg)
            .typeset(text::CAPTION)
            .font_weight(FontWeight::MEDIUM)
            .font_features(tabular())
            .whitespace_nowrap()
            .children(mark.map(|m| div().flex_none().size(px(14.)).flex().items_center().justify_center().child(m)))
            .child(self.label)
            .when_some(tooltip, |d, tip| {
                d.tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
            })
            .when_some(self.on_click, |d, f| d.cursor_pointer().on_hover(on_hover).on_click(move |e, w, cx| f(e, w, cx)))
            .with_spring(scoped(&self.id, "fill"), SpringAnimation::new(motion::CONTROL).to(hovered && clickable), move |d, t| {
                // An accent chip deepens its text instead of its fill.
                d.bg(mix(rest, under, t.0)).when(accent && t.0 > 0.5, |d| d.text_color(p.accent))
            })
    }
}
