//! Toast: a short message at the bottom center of the window ("Client
//! saved", "Couldn't save"). It takes no clicks: whatever lies under it can
//! still be clicked, so a message never holds up the next action. The one
//! exception is its `action` button (Undo, 2026-11-10). It rises in, stays
//! for `Toast::shown`, then fades; the caller drops it after that.

use std::rc::Rc;

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::*;

use super::motion::ease_out;
use crate::ui::components::icon;
use crate::ui::theme::{Elevation, RADIUS_CARD, Typeset as _, elevation, motion, palette, space, text};

/// The widest a message gets before it wraps.
const MAX_W: f32 = 440.0;
/// The rise in and the fade out.
const IN: f32 = 0.22;
const OUT: f32 = 0.35;

/// Something to run later with the window (a toast's button, a form saved).
pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;

/// A toast's button and what it does.
pub type ToastAction = (SharedString, Callback);

#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    text: SharedString,
    error: bool,
    action: Option<ToastAction>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self { id: id.into(), text: text.into(), error: false, action: None }
    }

    /// A button at the end (Undo); a toast with one stays a little longer.
    pub fn action(mut self, action: Option<ToastAction>) -> Self {
        self.action = action;
        self
    }

    /// How long a message with a button stays.
    pub fn shown_with_action() -> Duration {
        Duration::from_millis(7000)
    }

    /// A problem: red, and it stays longer.
    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    /// How long a message stays, fade included.
    pub fn shown(error: bool) -> Duration {
        Duration::from_millis(if error { 7000 } else { 4000 })
    }
}

impl RenderOnce for Toast {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let (bg, ink, mark) =
            if self.error { (p.negative, p.on_accent, IconName::CircleAlert) } else { (p.text, p.surface, IconName::CircleCheck) };
        let shown = if self.action.is_some() { Self::shown_with_action() } else { Self::shown(self.error) };
        let total = shown.as_secs_f32();
        let button = self.action.map(|(label, run)| {
            div()
                .id(ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "act".into()))
                .flex_none()
                .px(px(6.))
                .rounded(px(6.))
                .font_weight(FontWeight::BOLD)
                .underline()
                .cursor_pointer()
                .child(label)
                .on_click(move |_, window, cx| run(window, cx))
        });
        h_flex()
            .max_w(px(MAX_W))
            .items_start()
            .gap(px(10.))
            .px(px(space::M + 2.))
            .py(px(space::M))
            .rounded(px(RADIUS_CARD))
            .bg(bg)
            .text_color(ink)
            .typeset(text::BODY)
            .shadow(elevation(&p, Elevation::Floating))
            .child(div().pt(px(1.)).child(icon(mark, ink, 18.)))
            .child(div().flex_1().min_w_0().child(self.text))
            .children(button)
            .with_animation(self.id, Animation::new(shown), move |d, t| {
                let s = t * total;
                let rise = ease_out(s / IN);
                let fade = 1.0 - ((s - (total - OUT)) / OUT).clamp(0.0, 1.0);
                d.opacity(rise.min(fade)).relative().top(px(motion::ENTER_DISTANCE * (1.0 - rise)))
            })
    }
}
