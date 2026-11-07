//! Fab: the round "+" button at the bottom right. A click opens a stack of
//! what can be added above it, the primary action nearest the button
//! (approved 2026-11-05 with the redesign). It closes on a pick, Escape or
//! a click elsewhere. Put it in an absolutely placed corner of the page.

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ClickHandler;
use super::interact::{faded, hover_state, scoped};
use crate::ui::components::icon;
use crate::ui::theme::{Elevation, Typeset as _, elevation, motion, palette, text};

pub struct FabItem {
    label: SharedString,
    icon: IconName,
    hint: Option<SharedString>,
    primary: bool,
    on_select: ClickHandler,
}

impl FabItem {
    pub fn new(label: impl Into<SharedString>, icon: IconName, on_select: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        Self { label: label.into(), icon, hint: None, primary: false, on_select: std::rc::Rc::new(on_select) }
    }

    /// A keyboard shortcut shown after the label.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The action used most: filled, and nearest the button.
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }
}

#[derive(IntoElement)]
pub struct Fab {
    id: ElementId,
    tip: SharedString,
    items: Vec<FabItem>,
    start_open: bool,
}

impl Fab {
    pub fn new(id: impl Into<ElementId>, tip: impl Into<SharedString>) -> Self {
        Self { id: id.into(), tip: tip.into(), items: Vec::new(), start_open: false }
    }

    /// Shows the stack open (debug start screens, for screenshots).
    pub fn start_open(mut self, open: bool) -> Self {
        self.start_open = open;
        self
    }

    /// Items in order from the button upward.
    pub fn item(mut self, item: FabItem) -> Self {
        self.items.push(item);
        self
    }
}

impl RenderOnce for Fab {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let state = window.use_keyed_state(scoped(&self.id, "open"), cx, |_, _| false);
        let open = *state.read(cx) || self.start_open;
        let set = {
            let state = state.clone();
            move |v: bool, cx: &mut App| {
                state.update(cx, |s, cx| {
                    *s = v;
                    cx.notify();
                })
            }
        };
        let (over_button, on_button_hover) = hover_state(&scoped(&self.id, "button"), window, cx);
        let rows = self.items.into_iter().enumerate().rev().map(|(ix, item)| {
            let (ink, fill, chip_ink, chip_fill) = if item.primary {
                (p.on_accent, p.accent, p.on_accent, faded(p.on_accent, 0.18))
            } else {
                (p.text, p.surface, p.accent_text, p.accent_tint)
            };
            let (f, set) = (item.on_select.clone(), set.clone());
            let rid = ElementId::NamedChild(std::sync::Arc::new(scoped(&self.id, "item")), ix.to_string().into());
            let (hovered, on_hover) = hover_state(&rid, window, cx);
            h_flex()
                .id(rid.clone())
                .gap(px(10.))
                .pl(px(7.))
                .pr(px(14.))
                .py(px(7.))
                .rounded(px(12.))
                .border_1()
                .border_color(if item.primary { p.accent } else { p.border })
                .bg(if hovered { if item.primary { p.accent_hover } else { p.hover } } else { fill })
                .shadow(elevation(&p, Elevation::Floating))
                .cursor_pointer()
                .child(h_flex().size(px(32.)).rounded(px(9.)).justify_center().bg(chip_fill).child(icon(item.icon, chip_ink, 16.)))
                .child(div().typeset(text::BODY_STRONG).text_color(ink).child(item.label))
                .children(item.hint.map(|h| {
                    div().typeset(text::CAPTION).text_color(if item.primary { faded(p.on_accent, 0.8) } else { p.faint }).child(h)
                }))
                .on_hover(on_hover)
                .on_click(move |e, w, cx| {
                    set(false, cx);
                    f(e, w, cx);
                })
                // Each item rises into place.
                .with_spring(scoped(&rid, "rise"), SpringAnimation::new(motion::RISE).to(1.0f32).from(0.0), |d, t| {
                    d.opacity(t.clamp(0.0, 1.0)).mt(px(10. * (1.0 - t)))
                })
        });
        let stack = open.then(|| {
            let close = set.clone();
            v_flex()
                .id(scoped(&self.id, "stack"))
                .occlude()
                .absolute()
                .bottom(px(68.))
                .right_0()
                .items_end()
                .gap(px(8.))
                .children(rows)
                .on_mouse_down_out(move |_, _, cx| {
                    if !over_button {
                        close(false, cx)
                    }
                })
                .on_key_down({
                    let set = set.clone();
                    move |e: &KeyDownEvent, _, cx| {
                        if e.keystroke.key == "escape" {
                            set(false, cx)
                        }
                    }
                })
        });
        let tip = self.tip;
        let toggle = set.clone();
        let (hovered, on_hover) = hover_state(&scoped(&self.id, "fab"), window, cx);
        div().relative().flex_none().children(stack.map(|s| deferred(s).with_priority(1))).child(
            h_flex()
                .id(scoped(&self.id, "button"))
                .size(px(56.))
                .rounded_full()
                .justify_center()
                .bg(if hovered { p.accent_hover } else { p.accent })
                .shadow(elevation(&p, Elevation::Floating))
                .cursor_pointer()
                .on_hover(move |h, w, cx| {
                    on_hover(h, w, cx);
                    on_button_hover(h, w, cx);
                })
                .when(!open, |d| d.tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx)))
                .on_click(move |_, _, cx| toggle(!open, cx))
                .child(icon(IconName::Plus, p.on_accent, 24.).with_spring(
                    scoped(&self.id, "turn"),
                    SpringAnimation::new(motion::CONTROL).to(open),
                    |i, t| i.rotate(radians(std::f32::consts::FRAC_PI_4 * 3. * t.0)),
                )),
        )
    }
}
