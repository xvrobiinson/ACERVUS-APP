//! Splash: "Rising Bars", the launch splash (picked by the owner
//! 2026-11-03, the same as the phone app's). The logo tile pops in, its
//! three bars grow in turn, the name rises letter by letter, then the tile
//! zooms open and the splash fades onto the screen behind. It always plays
//! in full and is over in under 3.5 s. The caller removes it after
//! `Splash::SHOWN` and keeps its screen back until `Splash::OUT`, so the
//! screen's own arrival plays as the splash opens.

use std::f32::consts::TAU;
use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::interact::faded;
use super::motion::ease_out;
use crate::ui::theme::{SplashColors, Typeset as _, palette, text};

/// The tile's side, and the bars inside it on the logo's 512 grid:
/// (left, top, opacity); all are 72 wide and stand on y = 388.
const TILE: f32 = 112.0;
const BARS: [(f32, f32, f32); 3] = [(104., 270., 0.55), (220., 204., 0.8), (336., 124., 1.0)];
/// The line under the name.
const LINE_W: f32 = 148.0;

#[derive(IntoElement)]
pub struct Splash {
    id: ElementId,
}

impl Splash {
    /// When the intro ends and the tile starts to zoom open.
    pub const OUT: Duration = Duration::from_millis(2200);
    /// How long it stays, zoom and fade included.
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

/// Overshoots a little, then settles (the tile and bars arriving).
fn back(t: f32) -> f32 {
    bezier(0.34, 1.56, 0.64, 1.0, t)
}

/// Fast, then a long gentle settle (the letters rising).
fn expo(t: f32) -> f32 {
    bezier(0.16, 1.0, 0.3, 1.0, t)
}

/// A slow breath from 0 to 1 and back, `period` seconds long.
fn breath(s: f32, period: f32) -> f32 {
    0.5 - 0.5 * (s / period * TAU).cos()
}

/// The ground and its glow, centered on the tile's place.
fn sky(c: SplashColors, s: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            window.paint_quad(fill(b, c.edge));
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let vmin = w.min(h);
            let at = |y: f32| point(b.origin.x + px(w / 2.), b.origin.y + px(h * y));
            // Soft light that thins toward the edges, like a radial gradient.
            let glow = |center: Point<Pixels>, rx: f32, ry: f32, blur: f32, color: Hsla, window: &mut Window| {
                let shape = Bounds::centered_at(center, size(px(rx * 2.), px(ry * 2.)));
                let shadow = BoxShadow::new(px(0.), px(0.), color).blur_radius(px(blur));
                window.paint_drop_shadows(shape, Corners::all(px(rx.min(ry))), &[shadow]);
            };
            glow(at(0.42), w * 0.26, h * 0.2, vmin * 0.22, c.mid, window);
            glow(at(0.42), w * 0.1, h * 0.08, vmin * 0.16, c.center, window);
            let breathe = breath(s, 3.8);
            let r = vmin * 0.09 * (1.0 + 0.08 * breathe);
            glow(at(0.46), r, r, vmin * 0.13, faded(c.glow, 1.0 - 0.45 * breathe), window);
        },
    )
    .absolute()
    .inset_0()
}

/// The logo tile at `s` seconds: popping in, bars growing then breathing,
/// and from `Splash::OUT` zooming open.
fn tile(c: SplashColors, s: f32) -> impl IntoElement {
    let arrive = back(step(s, 0.0, 0.9));
    let zoom = step(s, Splash::OUT.as_secs_f32(), 1.0);
    let open = |t: f32| bezier(0.7, 0.0, 0.2, 1.0, t);
    let scale = if zoom <= 0.0 {
        0.45 + 0.55 * arrive
    } else if zoom < 0.22 {
        1.0 - 0.1 * open(zoom / 0.22)
    } else {
        0.9 + 23.1 * open((zoom - 0.22) / 0.78)
    };
    let alpha = arrive.clamp(0.0, 1.0) * if zoom < 0.7 { 1.0 } else { 1.0 - open((zoom - 0.7) / 0.3) };
    let bars: Vec<f32> = (0..BARS.len())
        .map(|k| {
            let grown = back(step(s, 0.32 + 0.15 * k as f32, 0.85)).max(0.0);
            let from = 1.8 + 0.18 * k as f32;
            let rest = if s > from { 1.0 - 0.16 * breath(s - from, 2.1) } else { 1.0 };
            grown * rest
        })
        .collect();
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let side = TILE * scale;
            let t = Bounds::centered_at(b.center(), size(px(side), px(side)));
            let unit = side / 512.;
            if zoom <= 0.0 {
                let shadow = BoxShadow::new(px(0.), px(18. * scale), faded(c.tile_shadow, alpha)).blur_radius(px(40. * scale));
                window.paint_drop_shadows(t, Corners::all(px(112. * unit)), &[shadow]);
            }
            window.paint_quad(fill(t, faded(c.tile, alpha)).corner_radii(px(112. * unit)));
            for (&(x, top, a), &grown) in BARS.iter().zip(&bars) {
                let tall = (388. - top) * grown;
                if tall <= 0.0 {
                    continue;
                }
                let bar = Bounds::new(
                    point(t.origin.x + px(x * unit), t.origin.y + px((388. - tall) * unit)),
                    size(px(72. * unit), px(tall * unit)),
                );
                window.paint_quad(fill(bar, faded(c.bar, a * alpha)).corner_radii(px(16f32.min(tall / 2.) * unit)));
            }
        },
    )
    .size(px(TILE))
}

impl RenderOnce for Splash {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = palette(cx).splash;
        let (word, sub) = crate::APP_NAME.split_once(' ').unwrap_or((crate::APP_NAME, ""));
        let (word, sub): (Vec<char>, Vec<char>) = (word.chars().collect(), sub.to_uppercase().chars().collect());
        let total = Self::SHOWN.as_secs_f32();
        let out = Self::OUT.as_secs_f32();
        div().id("splash-face").absolute().inset_0().overflow_hidden().occlude().with_animation(
            self.id,
            Animation::new(Self::SHOWN),
            move |d, t| {
                let s = t * total;
                // Everything under the tile steps down as it zooms open.
                let leave = step(s, out, 0.35);
                let drop = 10. * ease_out(step(s, out, 0.5));
                let soft = |t: f32| bezier(0.25, 0.1, 0.25, 1.0, t);
                let letters = word.iter().enumerate().map(|(i, ch)| {
                    let rise = expo(step(s, 0.76 + 0.056 * i as f32, 0.95));
                    div().relative().top(px(text::BRAND.line * 1.1 * (1. - rise))).child(ch.to_string())
                });
                let name = h_flex().overflow_hidden().pb(px(6.)).typeset(text::BRAND).text_color(c.ink).children(letters);
                let caption = soft(step(s, 1.2, 0.9));
                let sub = h_flex()
                    .gap(px(5.))
                    .mt(px(-8.))
                    .typeset(text::MICRO)
                    .text_color(c.sub)
                    .opacity(caption)
                    .relative()
                    .top(px(6. * (1. - caption)))
                    .children(sub.iter().map(|ch| div().child(ch.to_string())));
                let shown = soft(step(s, 1.32, 0.65));
                let filled = 0.08 + 0.92 * bezier(0.2, 0.8, 0.2, 1.0, step(s, 1.32, out - 1.32));
                let line = div()
                    .mt(px(10.))
                    .w(px(LINE_W))
                    .h(px(2.))
                    .rounded(px(2.))
                    .overflow_hidden()
                    .bg(c.track)
                    .opacity(shown)
                    .relative()
                    .top(px(6. * (1. - shown)))
                    .child(div().h_full().w(px(LINE_W * filled)).bg(linear_gradient(
                        90.,
                        linear_color_stop(c.line_from, 0.),
                        linear_color_stop(c.line_to, 1.),
                    )));
                let below =
                    v_flex().items_center().gap(px(18.)).opacity(1. - leave).relative().top(px(drop)).child(name).child(sub).child(line);
                let gone = bezier(0.2, 0.8, 0.2, 1.0, step(s, out + 0.45, 0.8));
                d.opacity(1. - gone)
                    .child(sky(c, s))
                    .child(v_flex().size_full().items_center().justify_center().gap(px(18.)).child(tile(c, s)).child(below))
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
        // The arriving curve overshoots before it settles.
        assert!(back(0.6) > 1.0);
    }

    #[test]
    fn it_is_over_in_under_three_and_a_half_seconds() {
        assert!(Splash::OUT < Splash::SHOWN);
        assert!(Splash::SHOWN <= Duration::from_millis(3500));
    }
}
