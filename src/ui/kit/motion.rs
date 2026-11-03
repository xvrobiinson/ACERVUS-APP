//! Motion for things that arrive or call attention once: a screen or row
//! rising into place, a field nudging side to side, a badge popping. Each
//! plays once when its element first appears under its id, follows the
//! theme's springs, and shows its end state at once when Windows' animation
//! effects are off.

use std::time::Duration;

use gpui_kit::*;

use crate::ui::theme::motion;

/// The RISE spring's settle curve as a timed animation that waits `delay`
/// before it starts.
pub fn arrival(delay: Duration) -> Animation {
    let (settle, curve) = sampled_easing(motion::RISE, 0.001);
    let total = delay + settle;
    let wait = delay.as_secs_f32() / total.as_secs_f32();
    Animation::new(total).with_easing(move |t| if t <= wait { 0.0 } else { curve((t - wait) / (1.0 - wait)) })
}

/// Eases out like a cubic: fast at first, settling gently.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// An element that fades in while rising `ENTER_DISTANCE` into place.
pub fn rise(el: impl IntoElement, id: impl Into<ElementId>, delay: Duration) -> AnyElement {
    div()
        .child(el)
        .with_animation(id, arrival(delay), |d, t| d.opacity(t.clamp(0.0, 1.0)).relative().top(px(motion::ENTER_DISTANCE * (1.0 - t))))
        .into_any_element()
}

/// Row `index` of a list that rises in one row after another. Rows past
/// `STAGGER_ROWS` are there at once.
pub fn rise_nth(el: impl IntoElement, id: impl Into<ElementId>, index: usize) -> AnyElement {
    if index >= motion::STAGGER_ROWS {
        return el.into_any_element();
    }
    rise(el, id, motion::STAGGER * index as u32)
}

/// Nudges an element side to side once, smaller each time (a wrong field).
pub fn shake(el: impl IntoElement, id: impl Into<ElementId>) -> AnyElement {
    div()
        .child(el)
        .with_animation(id, Animation::new(Duration::from_millis(360)), |d, t| {
            let swing = (t * std::f32::consts::PI * 4.0).sin() * 6.0 * (1.0 - t);
            d.relative().left(px(swing))
        })
        .into_any_element()
}

/// How much bigger something is during a pop: 0 at rest, up to 1 at the
/// peak, for a badge that just counted up.
pub fn pop(t: f32) -> f32 {
    if t < 0.4 { ease_out(t / 0.4) } else { 1.0 - ease_out((t - 0.4) / 0.6) }
}

/// A bell's swing (radians) at `t`: a few beats, each smaller.
pub fn swing(t: f32) -> f32 {
    (t * std::f32::consts::PI * 5.0).sin() * 0.28 * (1.0 - t).powi(2)
}
