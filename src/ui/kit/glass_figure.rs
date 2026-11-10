//! GlassFigure: one of the hero's four figures (the web's look, 2026-11-06):
//! a solid card over the fog or the streaks (the panel's surface, so it
//! matches the cards below) with a small label, a big thin amount whose cents are
//! faded, and up to two rows under it. A lone row reads larger. Overdue
//! amounts are red words, never a chip. Colors: `Palette::figure`.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::interact::{hover_state, scoped};
use crate::ui::components::sel;
use crate::ui::theme::{Elevation, Typeset as _, elevation, elevation_between, motion, palette, tabular, text};

/// How a row's value reads.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FigureTone {
    Plain,
    Good,
    Bad,
}

#[derive(IntoElement)]
pub struct GlassFigure {
    id: ElementId,
    label: Option<SharedString>,
    value: Option<SharedString>,
    action: Option<AnyElement>,
    rows: Vec<(SharedString, SharedString, FigureTone)>,
    body: Option<AnyElement>,
    compact: bool,
}

impl GlassFigure {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), label: None, value: None, action: None, rows: Vec::new(), body: None, compact: false }
    }

    /// The big figure and its label; a place can have only rows.
    pub fn big(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self.value = Some(value.into());
        self
    }

    /// Beside the label (Interest earned's period).
    pub fn action(mut self, el: impl IntoElement) -> Self {
        self.action = Some(el.into_any_element());
        self
    }

    /// Anything else in the card's place, in its colors (the figure picker
    /// while arranging).
    /// Tighter padding, for a smaller card (six in a row).
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn body(mut self, el: impl IntoElement) -> Self {
        self.body = Some(el.into_any_element());
        self
    }

    pub fn row(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>, tone: FigureTone) -> Self {
        self.rows.push((label.into(), value.into(), tone));
        self
    }
}

/// "RD$1,234.56" → ("RD$1,234", ".56"): the cents are drawn faded.
fn split_cents(s: &str) -> (String, String) {
    let b = s.as_bytes();
    let n = b.len();
    if n >= 3 && (b[n - 3] == b'.' || b[n - 3] == b',') && b[n - 2].is_ascii_digit() && b[n - 1].is_ascii_digit() {
        (s[..n - 3].to_string(), s[n - 3..].to_string())
    } else {
        (s.to_string(), String::new())
    }
}

impl RenderOnce for GlassFigure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let h = p.figure();
        let lone = self.rows.len() == 1;
        let compact = self.compact;
        let id = self.id.clone();
        let card = v_flex()
            .id(self.id)
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .px(px(if self.compact { 16. } else { 18. }))
            .pt(px(if self.compact { 14. } else { 16. }))
            .pb(px(if self.compact { 14. } else { 15. }))
            .rounded(px(18.))
            .bg(h.fill)
            // The clean look: flat cards with a thin edge, no shadow.
            .when(!self.compact, |d| d.shadow(elevation(&p, Elevation::Raised)))
            .border_1()
            .border_color(h.edge)
            .text_color(h.ink)
            .when_some(self.label.zip(self.value), |d, (label, value)| {
                let (whole, cents) = split_cents(&value);
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .h(px(18.))
                        .typeset(text::CAPTION)
                        .text_color(h.mute)
                        .child(div().min_w_0().truncate().child(sel(label)))
                        .children(self.action.map(|a| div().flex_none().h(px(18.)).flex().items_center().child(a))),
                )
                .child(
                    h_flex()
                        .mt(px(4.))
                        .items_baseline()
                        .typeset(text::HERO_FIGURE)
                        .font_features(tabular())
                        .whitespace_nowrap()
                        .child(sel(whole))
                        .child(div().text_color(h.cents).child(cents)),
                )
            })
            .when(!self.rows.is_empty(), |d| {
                d.child(v_flex().mt(px(12.)).gap(px(7.)).children(self.rows.into_iter().map(|(label, value, tone)| {
                    let color = match tone {
                        FigureTone::Plain => h.ink,
                        FigureTone::Good => h.good,
                        FigureTone::Bad => h.bad,
                    };
                    h_flex()
                        .justify_between()
                        .gap(px(10.))
                        .typeset(if lone { text::BODY } else { text::CAPTION })
                        .text_color(h.mute)
                        .child(div().min_w_0().truncate().child(sel(label)))
                        .child(
                            div()
                                .flex_none()
                                .font_weight(if tone == FigureTone::Bad { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                                .when(lone, |d| d.text_size(px(16.)))
                                .font_features(tabular())
                                .text_color(color)
                                .child(sel(value)),
                        )
                })))
            })
            .children(self.body);
        if !compact {
            return card.into_any_element();
        }
        // The six small cards rise a little under the pointer, with a soft
        // shadow (2026-11-10).
        let (hovered, on_hover) = hover_state(&id, window, cx);
        div()
            .id(scoped(&id, "lift-box"))
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .flex()
            .on_hover(on_hover)
            .with_spring(scoped(&id, "lift"), SpringAnimation::new(motion::CONTROL).to(hovered), move |d, t| {
                let t = t.0.clamp(0., 1.);
                d.relative().top(px(-3. * t)).child(card.shadow(elevation_between(&p, Elevation::Flat, Elevation::Lifted, t)))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::split_cents;

    #[test]
    fn cents_split_off_either_separator() {
        assert_eq!(split_cents("RD$1,234.56"), ("RD$1,234".into(), ".56".into()));
        assert_eq!(split_cents("Cg 1.250,00"), ("Cg 1.250".into(), ",00".into()));
        assert_eq!(split_cents("12"), ("12".into(), String::new()));
    }
}
