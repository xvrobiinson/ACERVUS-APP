//! Tour: the welcome tour's overlay (2026-11-10; the web's src/ui/tour.tsx).
//! A ring around what a step points at and a card beside it with the step's
//! title and words, "Step 2 of 6" with dots, Back, Next and Skip tutorial.
//! Nothing covers the app: only the card takes clicks.
//!
//! Elements a step can point at mark themselves with `tour_mark(key, …)`,
//! which records where they were drawn into `TourTargets`; the overlay reads
//! those places on the next frame.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Button, Callback};
use crate::i18n::t;
use crate::tr;
use crate::ui::theme::{Elevation, Typeset as _, elevation, palette, space, text};

/// Where each marked element was drawn, by its key: the last frame's
/// (read) and this frame's (written as it is drawn), so something no longer
/// shown is forgotten after one frame.
#[derive(Clone, Default)]
pub struct TourTargets(Rc<RefCell<Places>>);

#[derive(Default)]
struct Places {
    seen: HashMap<&'static str, Bounds<Pixels>>,
    now: HashMap<&'static str, Bounds<Pixels>>,
}

impl TourTargets {
    pub fn get(&self, key: &str) -> Option<Bounds<Pixels>> {
        self.0.borrow().seen.get(key).copied()
    }
    /// A new frame: what was drawn becomes what is read.
    pub fn next_frame(&self) {
        let mut p = self.0.borrow_mut();
        p.seen = std::mem::take(&mut p.now);
    }
}

/// An empty layer over its parent (which must be \`relative\`) that records
/// where the parent was drawn, for the tour to point at.
pub fn tour_mark(key: &'static str, targets: &TourTargets) -> impl IntoElement {
    let targets = targets.clone();
    canvas(
        move |bounds, _, _| {
            targets.0.borrow_mut().now.insert(key, bounds);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// One step: what it points at (the first key drawn), or nothing for a card
/// in the middle.
pub struct TourStep {
    pub targets: &'static [&'static str],
    pub title: &'static str,
    pub text: &'static str,
}

/// The card's width, and the room kept around what it points at.
const CARD_W: f32 = 340.;
const PAD: f32 = 6.;
/// About how tall a card is, to choose above or below its target.
const CARD_H: f32 = 190.;

/// Moves the tour to a step.
type GoTo = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Tour {
    steps: Rc<Vec<TourStep>>,
    at: usize,
    targets: TourTargets,
    on_at: GoTo,
    on_skip: Callback,
    on_finish: Callback,
    settled: bool,
    backward: bool,
}

impl Tour {
    pub fn new(
        steps: Rc<Vec<TourStep>>,
        at: usize,
        targets: TourTargets,
        on_at: impl Fn(usize, &mut Window, &mut App) + 'static,
        on_skip: Callback,
        on_finish: Callback,
    ) -> Self {
        Self { steps, at, targets, on_at: Rc::new(on_at), on_skip, on_finish, settled: true, backward: false }
    }

    /// False for the first frames after the screen changed: what the steps
    /// point at may not be read yet, so none is skipped as missing.
    pub fn settled(mut self, settled: bool) -> Self {
        self.settled = settled;
        self
    }

    /// The person went Back: a step with nothing to point at is skipped
    /// backwards.
    pub fn backward(mut self, backward: bool) -> Self {
        self.backward = backward;
        self
    }

    /// Where step \`at\` points, if it points at something drawn.
    fn target(&self, at: usize) -> Option<Bounds<Pixels>> {
        self.steps.get(at)?.targets.iter().find_map(|k| self.targets.get(k))
    }
}

impl RenderOnce for Tour {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let n = self.steps.len();
        let at = self.at.min(n.saturating_sub(1));
        let step = &self.steps[at];
        let last = at + 1 == n;
        let counted = n.saturating_sub(2);
        let target = self.target(at);
        // A step that points at something not on this screen (hidden by
        // Arrange, say) moves on by itself.
        if self.settled && !step.targets.is_empty() && target.is_none() && !last {
            let on_at = self.on_at.clone();
            let to = if self.backward && at > 0 { at - 1 } else { at + 1 };
            window.on_next_frame(move |w, cx| on_at(to, w, cx));
        }
        let viewport = window.viewport_size();
        let (vw, vh) = (f32::from(viewport.width), f32::from(viewport.height));

        let progress = (at > 0 && !last).then(|| {
            h_flex()
                .justify_between()
                .gap(px(space::S))
                .typeset(text::CAPTION)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.faint)
                .child(tr!("Step {at} of {counted}", "Paso {at} de {counted}"))
                .child(h_flex().gap(px(4.)).children((1..=counted).map(|i| {
                    div().h(px(6.)).w(px(if i == at { 16. } else { 6. })).rounded(px(3.)).bg(if i == at {
                        p.text
                    } else if i < at {
                        p.faint
                    } else {
                        p.border
                    })
                })))
        });
        let (on_at, skip, finish) = (self.on_at.clone(), self.on_skip.clone(), self.on_finish.clone());
        let back = self.on_at.clone();
        let acts = h_flex()
            .gap(px(space::S))
            .pt(px(space::M))
            .when(!last, |d| {
                d.child(
                    div()
                        .id("tour-skip")
                        .typeset(text::CAPTION)
                        .text_color(p.muted)
                        .underline()
                        .cursor_pointer()
                        .child(t("Skip tutorial", "Saltar el recorrido"))
                        .on_click(move |_, w, cx| skip(w, cx)),
                )
            })
            .child(div().flex_1())
            .when(at > 1 && !last, |d| {
                d.child(Button::new("tour-back").small().label(t("Back", "Atrás")).on_click(move |_, w, cx| back(at - 1, w, cx)))
            })
            .child(if last {
                Button::new("tour-finish").small().primary().label(t("Finish", "Terminar")).on_click(move |_, w, cx| finish(w, cx))
            } else {
                Button::new("tour-next")
                    .small()
                    .primary()
                    .label(if at == 0 { t("Start", "Empezar") } else { t("Next", "Siguiente") })
                    .on_click(move |_, w, cx| on_at(at + 1, w, cx))
            });
        let card = v_flex()
            .id("tour-card")
            .absolute()
            .w(px(CARD_W))
            .p(px(space::L))
            .gap(px(6.))
            .rounded(px(16.))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .shadow(elevation(&p, Elevation::Floating))
            .occlude()
            .children(progress)
            .child(div().typeset(text::HEADING).text_color(p.text).child(step.title))
            .child(div().typeset(text::BODY).text_color(p.muted).child(step.text))
            .child(acts);
        // Beside the target: under it when there is room, else above it.
        let card = match target {
            Some(b) => {
                let (x, y, w, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
                let left = (x + w / 2. - CARD_W / 2.).clamp(12., (vw - CARD_W - 12.).max(12.));
                let card = card.left(px(left));
                if y + h + 14. + CARD_H < vh { card.top(px(y + h + 14.)) } else { card.bottom(px((vh - y + 14.).max(12.))) }
            }
            None => card.left(px((vw - CARD_W) / 2.)).top(px((vh - CARD_H) / 2.)),
        };
        let ring = target.map(|b| {
            div()
                .absolute()
                .left(b.origin.x - px(PAD))
                .top(b.origin.y - px(PAD))
                .w(b.size.width + px(PAD * 2.))
                .h(b.size.height + px(PAD * 2.))
                .rounded(px(16.))
                .border_2()
                .border_color(p.text)
                .with_animation(
                    ElementId::NamedChild(std::sync::Arc::new(ElementId::Name("tour-ring".into())), at.to_string().into()),
                    Animation::new(std::time::Duration::from_millis(2000)).repeat(),
                    move |d, t| {
                        let glow = 0.18 + 0.1 * (std::f32::consts::PI * 2. * t).sin();
                        d.shadow(vec![BoxShadow {
                            color: hsla(0., 0., if p.dark { 1. } else { 0. }, glow),
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(22.),
                            spread_radius: px(4.),
                            inset: false,
                        }])
                    },
                )
        });
        div().absolute().inset_0().children(ring).child(card)
    }
}
