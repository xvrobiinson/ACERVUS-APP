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

/// A bar that fills from the left the first time it appears under `id`
/// (the Overview's money and status bars, 2026-11-10). Its parts are flex
/// shares of its width, so they grow together.
pub fn fill_in(bar: Div, id: impl Into<ElementId>) -> AnyElement {
    let wait = 0.15;
    bar.with_animation(id, Animation::new(Duration::from_millis(1250)), move |d, t| {
        let t = if t <= wait { 0.0 } else { ease_out((t - wait) / (1.0 - wait)) };
        d.w(relative(t.max(0.002)))
    })
    .into_any_element()
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

/// A row that rises in as row `index` of its list, and glows `tint` for a
/// moment behind its content when it was just added.
pub fn row_in(row: impl IntoElement, id: impl Into<ElementId>, index: usize, glow: Option<Hsla>) -> AnyElement {
    let id = id.into();
    let row = match glow {
        Some(tint) => div()
            .child(row)
            .with_animation(super::interact::scoped(&id, "glow"), Animation::new(motion::GLOW), move |d, t| {
                let a = if t < 0.35 { 1.0 } else { 1.0 - ease_out((t - 0.35) / 0.65) };
                d.bg(super::interact::faded(tint, a))
            })
            .into_any_element(),
        None => row.into_any_element(),
    };
    rise_nth(row, super::interact::scoped(&id, "rise"), index)
}

/// How long a removed row takes to slide out and close its gap.
pub const FOLD: Duration = Duration::from_millis(440);

/// A row on its way out: it slides right while fading, then its height
/// closes so the rows below move up.
pub fn fold_away(el: impl IntoElement, id: impl Into<ElementId>) -> AnyElement {
    div()
        .overflow_hidden()
        .child(el)
        .with_animation(id, Animation::new(FOLD), |d, t| {
            let out = ease_out((t / 0.5).min(1.0));
            let close = ease_out(((t - 0.5) / 0.5).max(0.0));
            d.opacity(1.0 - out).relative().left(px(48.0 * out)).max_h(px(72.0 * (1.0 - close)))
        })
        .into_any_element()
}

/// Content that slides in sideways while fading: from the right for a page
/// opened (`dir` 1), from the left for one gone back to (-1), in place (0).
pub fn slide(el: impl IntoElement, id: impl Into<ElementId>, dir: i8) -> AnyElement {
    div()
        .child(el)
        .with_animation(id, arrival(Duration::ZERO), move |d, t| {
            d.opacity(t.clamp(0.0, 1.0)).relative().left(px(24.0 * f32::from(dir) * (1.0 - t)))
        })
        .into_any_element()
}
