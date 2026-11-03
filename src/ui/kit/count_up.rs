//! CountUp: an amount that counts up from zero when it first appears, then
//! rolls to each new amount with a brief green (up) or red (down) flash.
//! Settled, it shows the exact amount as selectable text.

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::*;

use super::interact::{mix, scoped};
use super::motion::ease_out;
use crate::ui::components::sel;
use crate::ui::theme::{motion, palette};

#[derive(IntoElement)]
pub struct CountUp {
    id: ElementId,
    value: i64,
    format: Rc<dyn Fn(i64) -> String>,
    color: Option<Hsla>,
}

/// What was shown last, so a new amount rolls from it.
struct Shown {
    from: i64,
    to: i64,
    first: bool,
}

impl CountUp {
    pub fn new(id: impl Into<ElementId>, value: i64, format: impl Fn(i64) -> String + 'static) -> Self {
        Self { id: id.into(), value, format: Rc::new(format), color: None }
    }

    /// The text color at rest; the flash fades back to it.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for CountUp {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let value = self.value;
        let state = window.use_keyed_state(scoped(&self.id, "shown"), cx, |_, _| Shown { from: 0, to: value, first: true });
        let (from, first) = state.update(cx, |s, _| {
            if s.to != value {
                *s = Shown { from: s.to, to: value, first: false };
            }
            (s.from, s.first)
        });
        // A new id per amount restarts the animation when the amount changes.
        let key = ElementId::NamedChild(Arc::new(self.id), value.to_string().into());
        let rest = self.color.unwrap_or(p.text);
        let flash = if value >= from { p.positive } else { p.negative };
        let format = self.format;
        let animation =
            if first { Animation::new(motion::COUNT_UP).with_easing(ease_out) } else { Animation::new(motion::GLOW).with_easing(|t| t) };
        div().with_animation(key, animation, move |d, t| {
            // The roll takes ROLL; the flash fades over the whole GLOW.
            let roll = if first { t } else { ease_out(t * motion::GLOW.as_secs_f32() / motion::ROLL.as_secs_f32()) };
            let shown = if roll >= 1.0 { value } else { from + ((value - from) as f64 * f64::from(roll)).round() as i64 };
            let d = d.child(sel(format(shown)));
            if first { d } else { d.text_color(mix(flash, rest, ease_out(t))) }
        })
    }
}
