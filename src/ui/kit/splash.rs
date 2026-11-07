//! Splash: "Orbit", the launch splash (picked by the owner 2026-11-05, the
//! same as the web app's). Over the hero's picture the black tile pops in,
//! the dot circles the faint ring and lands in its gap as the ring lights
//! up, the name rises letter by letter and a line fills. Then it opens
//! "through the ring": the ring grows toward you, past the window's
//! corners, while the picture fades onto the screen behind. It always plays
//! in full and is over in under 3.5 s. The caller removes it after
//! `Splash::SHOWN` and keeps its screen back until `Splash::OUT`, so the
//! screen's own arrival plays as the splash opens.

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::interact::faded;
use super::logo::{MarkLook, RING_CENTER, RING_HOLE, paint_mark};
use crate::ui::theme::{SplashColors, Typeset as _, palette, text};

/// The tile's side.
const TILE: f32 = 112.0;
/// The line under the name.
const LINE_W: f32 = 110.0;

#[derive(IntoElement)]
pub struct Splash {
    id: ElementId,
}

impl Splash {
    /// When the intro ends and the splash starts to open.
    pub const OUT: Duration = Duration::from_millis(2200);
    /// How long it stays, the opening included.
    pub const SHOWN: Duration = Duration::from_millis(3450);

    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}

/// A CSS cubic-bezier(x1, y1, x2, y2) easing at `t`.
fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let at = |a: f32, b: f32, s: f32| 3.0 * a * s * (1.0 - s).powi(2) + 3.0 * b * s * s * (1.0 - s) + s.powi(3);
    // x grows with s on every curve used here, so halving finds it.
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if at(x1, x2, mid) < t { lo = mid } else { hi = mid }
    }
    at(y1, y2, (lo + hi) / 2.0)
}

/// Progress of a step that starts at `from` seconds and lasts `len`.
fn step(s: f32, from: f32, len: f32) -> f32 {
    ((s - from) / len).clamp(0.0, 1.0)
}

/// Quick, then a long soft landing (the web's cubic-bezier(0.2, 0.8, 0.2, 1)).
fn settle(t: f32) -> f32 {
    bezier(0.2, 0.8, 0.2, 1.0, t)
}

/// Slow, fast, slow (the opening).
fn in_out(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// The tile's size at `s`: it pops in a little past its size, then settles.
fn pop(s: f32) -> (f32, f32) {
    let e = settle(step(s, 0.0, 0.45));
    let scale = if e < 0.6 { 0.6 + 0.45 * (e / 0.6) } else { 1.05 - 0.05 * ((e - 0.6) / 0.4) };
    (scale, (e / 0.6).min(1.0))
}

/// The mark at `s` seconds, and from `Splash::OUT` the opening through the
/// ring: it grows about the ring's center until the hole passes the
/// window's farthest corner.
fn mark(c: SplashColors, s: f32) -> impl IntoElement {
    let (scale, shown) = pop(s);
    let orbit = bezier(0.35, 0.05, 0.2, 1.0, step(s, 0.25, 1.5));
    let lit = 0.22 + 0.78 * step(s, 1.5, 0.45);
    let opening = in_out(step(s, Splash::OUT.as_secs_f32(), 1.0));
    let tile_gone = step(s, Splash::OUT.as_secs_f32(), 0.35);
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let side = TILE * scale;
            let tile = Bounds::centered_at(b.center(), size(px(side), px(side)));
            // How far the hole must grow: from the ring's center to the
            // window's farthest corner.
            let view = window.viewport_size();
            let unit = side / 512.;
            let (rx, ry) = (f32::from(tile.origin.x) + RING_CENTER.0 * unit, f32::from(tile.origin.y) + RING_CENTER.1 * unit);
            let (w, h) = (f32::from(view.width), f32::from(view.height));
            let far = [(0., 0.), (w, 0.), (0., h), (w, h)].iter().map(|(x, y)| (x - rx).hypot(y - ry)).fold(0., f32::max) + 4.;
            let grow = far / (RING_HOLE * unit);
            let gone = 1.0 - step(opening, 0.7, 0.3);
            let ring_alpha = lit * shown * gone;
            paint_mark(
                window,
                tile,
                MarkLook {
                    tile: faded(c.tile, shown * (1.0 - tile_gone)),
                    edge: c.tile_line.map(|e| faded(e, e.a * shown * (1.0 - tile_gone))),
                    ring: faded(c.ring, ring_alpha),
                    dot: faded(c.ring, shown * gone),
                    dot_turn: -430. * (1.0 - orbit),
                    zoom: 1.0 + (grow - 1.0) * opening,
                },
            );
        },
    )
    .size(px(TILE))
}

impl RenderOnce for Splash {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let c = p.splash;
        let picture = crate::ui::pictures::hero(p.dark);
        let (word, sub) = crate::APP_NAME.split_once(' ').unwrap_or((crate::APP_NAME, ""));
        let (word, sub): (Vec<char>, Vec<char>) = (word.chars().collect(), sub.to_uppercase().chars().collect());
        let total = Self::SHOWN.as_secs_f32();
        let out = Self::OUT.as_secs_f32();
        div().id("splash-face").absolute().inset_0().overflow_hidden().occlude().with_animation(
            self.id,
            Animation::new(Self::SHOWN),
            move |d, t| {
                let s = t * total;
                // The words step away as the splash opens.
                let leave = 1.0 - step(s, out, 0.25);
                let letters = word.iter().enumerate().map(|(i, ch)| {
                    let rise = settle(step(s, 1.6 + 0.045 * i as f32, 0.6));
                    div().relative().top(px(48. * 1.1 * (1. - rise))).opacity(rise).child(ch.to_string())
                });
                let name = h_flex()
                    .overflow_hidden()
                    .mt(px(10.))
                    .pb(px(6.))
                    .typeset(text::HERO_FIGURE)
                    .text_size(px(44.))
                    .line_height(px(48.))
                    .text_color(c.ink)
                    .children(letters);
                let caption = settle(step(s, 2.0, 0.6));
                let sub = h_flex()
                    .gap(px(4.5))
                    .mt(px(-6.))
                    .typeset(text::MICRO)
                    .font_weight(FontWeight::NORMAL)
                    .text_color(c.sub)
                    .opacity(caption)
                    .relative()
                    .top(px(6. * (1. - caption)))
                    .children(sub.iter().map(|ch| div().child(ch.to_string())));
                let shown = settle(step(s, 0.2, 0.5));
                let filled = 0.08 + 0.92 * settle(step(s, 0.2, out - 0.2));
                let line = div()
                    .mt(px(8.))
                    .w(px(LINE_W))
                    .h(px(2.))
                    .rounded(px(2.))
                    .overflow_hidden()
                    .bg(c.track)
                    .opacity(shown)
                    .relative()
                    .top(px(6. * (1. - shown)))
                    .child(div().h_full().w(px(LINE_W * filled)).bg(c.ink));
                let below = v_flex().items_center().gap(px(10.)).opacity(leave).child(name).child(sub).child(line);
                // The picture gives way as the ring opens, and is gone when
                // the ring passes the corners.
                let opening = in_out(step(s, out, 1.0));
                let ground = (1.0 - opening).clamp(0.0, 1.0);
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(c.base)
                        .opacity(ground)
                        .child(img(picture.clone()).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)),
                )
                .child(v_flex().size_full().items_center().justify_center().gap(px(4.)).child(mark(c, s)).child(below))
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn bezier_matches_its_ends_and_the_linear_curve() {
        assert!(bezier(0.7, 0.0, 0.2, 1.0, 0.0).abs() < 1e-4);
        assert!((bezier(0.7, 0.0, 0.2, 1.0, 1.0) - 1.0).abs() < 1e-4);
        assert!((bezier(1. / 3., 1. / 3., 2. / 3., 2. / 3., 0.4) - 0.4).abs() < 1e-3);
    }

    #[test]
    fn the_tile_pops_past_its_size_then_settles() {
        assert!((pop(0.0).0 - 0.6).abs() < 1e-3);
        assert!((pop(1.0).0 - 1.0).abs() < 1e-3);
        assert!((0..45).map(|i| pop(i as f32 / 100.).0).fold(0., f32::max) > 1.0);
    }

    #[test]
    fn it_is_over_in_under_three_and_a_half_seconds() {
        assert!(Splash::OUT < Splash::SHOWN);
        assert!(Splash::SHOWN <= Duration::from_millis(3500));
        // The opening (1 s) and the picture's fade end within the time shown.
        assert!(Splash::OUT + Duration::from_millis(1000) <= Splash::SHOWN);
    }
}
