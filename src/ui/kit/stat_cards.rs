//! Stat cards: each [`Stat`] on its own card, `per_row` to a row. The last
//! row keeps the same cell width as the rows above it. Figures never shrink
//! to fit; callers lower `per_row` instead. A card lifts a little under
//! the pointer.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use std::rc::Rc;
use std::sync::Arc;

use super::interact::{hover_state, scoped};
use super::{Card, Glide, Stat};
use crate::ui::theme::{motion, space};

/// Adds behavior to the card at a place.
type Frame = Rc<dyn Fn(usize, Stateful<Div>) -> Stateful<Div>>;

#[derive(IntoElement)]
pub struct StatCards {
    /// Set: cards lift under the pointer (ids are scoped under it).
    lifting: Option<ElementId>,
    /// A name per card, so a card keeps its state (and slides) when the
    /// order changes. Cards are numbered otherwise.
    keys: Vec<SharedString>,
    /// Extra behavior on each card (dragging while arranging), by place.
    frame: Option<Frame>,
    /// Cards slide to their new places when the order changes now.
    glide: bool,
    per_row: usize,
    spacing: f32,
    stats: Vec<Stat>,
}

impl StatCards {
    pub fn new(per_row: usize) -> Self {
        Self { lifting: None, keys: Vec::new(), frame: None, glide: false, per_row: per_row.max(1), spacing: space::L, stats: Vec::new() }
    }

    /// Cards rise a little, with a deeper shadow, under the pointer.
    pub fn lifting(mut self, id: impl Into<ElementId>) -> Self {
        self.lifting = Some(id.into());
        self
    }

    /// One name per card, in order (see `keys`).
    pub fn keys(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// Adds behavior to each lifting card (`i` is its place).
    pub fn frame(mut self, f: impl Fn(usize, Stateful<Div>) -> Stateful<Div> + 'static) -> Self {
        self.frame = Some(Rc::new(f));
        self
    }

    pub fn glide(mut self, glide: bool) -> Self {
        self.glide = glide;
        self
    }

    /// Space between cards (the screen's card gap).
    pub fn spacing(mut self, px: f32) -> Self {
        self.spacing = px;
        self
    }

    pub fn stats(mut self, stats: impl IntoIterator<Item = Stat>) -> Self {
        self.stats.extend(stats);
        self
    }
}

impl RenderOnce for StatCards {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let gap = px(self.spacing);
        let per_row = self.per_row;
        let mut cells: Vec<AnyElement> = self
            .stats
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let card = Card::new().flex_1().flex_basis(px(0.)).min_w_0().p(px(space::L + 2.));
                let Some(parent) = &self.lifting else { return card.child(s).into_any_element() };
                let name = self.keys.get(i).cloned().unwrap_or_else(|| i.to_string().into());
                let id = ElementId::NamedChild(Arc::new(parent.clone()), name);
                let (hovered, on_hover) = hover_state(&id, window, cx);
                let wrapper = div().id(id.clone()).flex_1().flex_basis(px(0.)).min_w_0().flex().on_hover(on_hover);
                let wrapper = match &self.frame {
                    Some(f) => f(i, wrapper),
                    None => wrapper,
                };
                let card = wrapper.with_spring(scoped(&id, "lift"), SpringAnimation::new(motion::CONTROL).to(hovered), move |d, t| {
                    d.relative().top(px(-2. * t.0)).child(card.lift(t.0.clamp(0., 1.)).child(s))
                });
                Glide::new(scoped(&id, "glide"), card).armed(self.glide).into_any_element()
            })
            .collect();
        let mut rows = Vec::new();
        while !cells.is_empty() {
            let rest = cells.split_off(per_row.min(cells.len()));
            let short = per_row - cells.len();
            rows.push(h_flex().items_stretch().gap(gap).children(cells).children((0..short).map(|_| div().flex_1().flex_basis(px(0.)))));
            cells = rest;
        }
        v_flex().gap(gap).children(rows)
    }
}
