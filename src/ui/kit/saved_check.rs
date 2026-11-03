//! SavedCheck: a green circle and check that draw themselves in the middle
//! of the window for a moment after a payment is saved, then fade away.
//! The caller removes it after `SavedCheck::SHOWN`.

use std::time::Duration;

use gpui_kit::component::v_flex;
use gpui_kit::*;

use super::motion::ease_out;
use crate::ui::theme::{Elevation, RADIUS_CARD, Typeset as _, elevation, palette, space, text};

#[derive(IntoElement)]
pub struct SavedCheck {
    id: ElementId,
    label: SharedString,
}

impl SavedCheck {
    /// How long it stays, fade included.
    pub const SHOWN: Duration = Duration::from_millis(1500);

    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// Phase `t` of the whole showing, mapped into `a..b` and eased.
fn span(t: f32, a: f32, b: f32) -> f32 {
    ease_out((t - a) / (b - a))
}

impl RenderOnce for SavedCheck {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let total = Self::SHOWN.as_secs_f32();
        let label = self.label;
        div().absolute().inset_0().flex().items_center().justify_center().child(
            v_flex()
                .items_center()
                .gap(px(space::S))
                .px(px(space::XL))
                .py(px(space::L))
                .rounded(px(RADIUS_CARD))
                .bg(p.surface)
                .border_1()
                .border_color(p.border)
                .shadow(elevation(&p, Elevation::Floating))
                .with_animation(self.id, Animation::new(Self::SHOWN), move |d, t| {
                    let ring = span(t, 0.0, 0.38 / total);
                    let tick = span(t, 0.26 / total, 0.58 / total);
                    let words = span(t, 0.3 / total, 0.56 / total);
                    let fade = 1.0 - span(t, (total - 0.25) / total, 1.0);
                    let (green, tint) = (p.positive, p.positive_tint);
                    let mark = canvas(
                        |_, _, _| {},
                        move |b, _, window, _| {
                            let c = b.center();
                            let r = b.size.width / 2.0 - px(2.);
                            if ring > 0.0 {
                                let mut fill_c = tint;
                                fill_c.a *= ring;
                                window.paint_quad(fill(Bounds::centered_at(c, size(r * 2., r * 2.)), fill_c).corner_radii(r));
                                let mut b = PathBuilder::stroke(px(2.5));
                                let steps = (48.0 * ring).ceil() as usize;
                                for k in 0..=steps {
                                    // From the top, clockwise.
                                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * ring * k as f32 / steps.max(1) as f32;
                                    let pt = point(c.x + r * a.cos(), c.y + r * a.sin());
                                    if k == 0 { b.move_to(pt) } else { b.line_to(pt) }
                                }
                                if let Ok(path) = b.build() {
                                    window.paint_path(path, green);
                                }
                            }
                            if tick > 0.0 {
                                let s = f32::from(r) / 23.0;
                                let at = |x: f32, y: f32| point(c.x + px((x - 26.) * s), c.y + px((y - 26.) * s));
                                let (a, m, z) = (at(15., 27.), at(22., 34.), at(37., 18.));
                                // Two legs: the short one takes the first third.
                                let mut b = PathBuilder::stroke(px(3.5 * s));
                                b.move_to(a);
                                let first = (tick / 0.35).min(1.0);
                                b.line_to(a + (m - a) * first);
                                if tick > 0.35 {
                                    b.line_to(m + (z - m) * ((tick - 0.35) / 0.65));
                                }
                                if let Ok(path) = b.build() {
                                    window.paint_path(path, green);
                                }
                            }
                        },
                    )
                    .size(px(52.));
                    d.opacity(fade).child(mark).child(
                        div()
                            .typeset(text::BODY_STRONG)
                            .text_color(p.text)
                            .opacity(words)
                            .relative()
                            .top(px(4. * (1. - words)))
                            .child(label.clone()),
                    )
                }),
        )
    }
}
