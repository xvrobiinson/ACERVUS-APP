//! The Acervus mark (2026-11-05, the same as the web app's): a black
//! rounded tile with a white ring open at its top right, where a dot sits.
//! Drawn on the logo's 512 grid: the ring is centered at (255, 270) with a
//! radius of 133 and a stroke of 54, the dot at (357, 184) with a radius of
//! 27. `paint_mark` draws it at any size, with the launch splash's motion
//! (the dot's orbit, the ring growing toward you); `Logo` is the still mark.

use gpui_kit::*;

/// Where the ring is centered, on the 512 grid.
pub const RING_CENTER: (f32, f32) = (255., 270.);
/// The ring's hole, out to the inside of the stroke.
pub const RING_HOLE: f32 = 133. - 27.;

/// How the mark is drawn this frame.
#[derive(Clone, Copy)]
pub struct MarkLook {
    pub tile: Hsla,
    /// A thin line around the tile (dark backgrounds), if any.
    pub edge: Option<Hsla>,
    pub ring: Hsla,
    /// The dot: solid while the ring is still faint (the splash).
    pub dot: Hsla,
    /// The dot's turn about the ring's center, in degrees (clockwise).
    pub dot_turn: f32,
    /// Growth about the ring's center (1: as laid out).
    pub zoom: f32,
}

impl MarkLook {
    /// The still mark: black tile, white ring and dot.
    pub fn still() -> Self {
        Self { tile: rgb(0x000000).into(), edge: None, ring: rgb(0xFFFFFF).into(), dot: rgb(0xFFFFFF).into(), dot_turn: 0., zoom: 1. }
    }
}

fn disc(window: &mut Window, center: Point<Pixels>, d: f32, color: Hsla) {
    window.paint_quad(fill(Bounds::centered_at(center, size(px(d), px(d))), color).corner_radii(px(d / 2.)));
}

/// Draws the mark in `tile` (a square), as `look` says.
pub fn paint_mark(window: &mut Window, tile: Bounds<Pixels>, look: MarkLook) {
    let side = f32::from(tile.size.width);
    let unit = side / 512.;
    let (cx, cy) = RING_CENTER;
    // Everything grows about the ring's center.
    let center = point(tile.origin.x + px(cx * unit), tile.origin.y + px(cy * unit));
    let u = unit * look.zoom;
    let at = |x: f32, y: f32| point(center.x + px((x - cx) * u), center.y + px((y - cy) * u));
    if look.tile.a > 0.0 || look.edge.is_some() {
        let b = Bounds::from_corners(at(0., 0.), at(512., 512.));
        let mut q = fill(b, look.tile).corner_radii(px(112. * u));
        if let Some(edge) = look.edge {
            q = q.border_widths(px((5. * u).max(1.))).border_color(edge);
        }
        window.paint_quad(q);
    }
    // The ring: an open arc with round ends.
    let (from, to) = ((386.7, 251.5), (300.5, 145.));
    let mut b = PathBuilder::stroke(px(54. * u));
    b.move_to(at(from.0, from.1));
    b.arc_to(point(px(133. * u), px(133. * u)), px(0.), true, true, at(to.0, to.1));
    if let Ok(path) = b.build() {
        window.paint_path(path, look.ring);
    }
    disc(window, at(from.0, from.1), 54. * u, look.ring);
    disc(window, at(to.0, to.1), 54. * u, look.ring);
    // The dot, turned about the ring's center.
    let (sin, cos) = look.dot_turn.to_radians().sin_cos();
    let (dx, dy) = (357. - cx, 184. - cy);
    disc(window, at(cx + dx * cos - dy * sin, cy + dx * sin + dy * cos), 54. * u, look.dot);
}

/// The still mark, `size` pixels square.
#[derive(IntoElement)]
pub struct Logo {
    size: f32,
    edge: Option<Hsla>,
    colors: Option<(Hsla, Hsla)>,
}

impl Logo {
    pub fn new(size: f32) -> Self {
        Self { size, edge: None, colors: None }
    }

    /// A thin line around the tile, so it shows on a dark background.
    pub fn edge(mut self, color: Hsla) -> Self {
        self.edge = Some(color);
        self
    }

    /// The tile's fill and the ring and dot's ink, where the black tile
    /// isn't the screen's (the signed-out screens' Deep Emerald).
    pub fn colors(mut self, tile: Hsla, ink: Hsla) -> Self {
        self.colors = Some((tile, ink));
        self
    }

    pub(crate) fn look(&self) -> MarkLook {
        let still = MarkLook::still();
        let (tile, ink) = self.colors.unwrap_or((still.tile, still.ring));
        MarkLook { edge: self.edge, tile, ring: ink, dot: ink, ..still }
    }
}

impl RenderOnce for Logo {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let look = self.look();
        canvas(|_, _, _| {}, move |b, _, window, _| paint_mark(window, b, look)).flex_none().size(px(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn the_mark_takes_the_colors_it_is_given() {
        let (tile, ink): (Hsla, Hsla) = (rgb(0x0F3329).into(), rgb(0xF5F5F0).into());
        let look = Logo::new(24.).colors(tile, ink).look();
        assert_eq!((look.tile, look.ring, look.dot, look.edge), (tile, ink, ink, None));
        let still = MarkLook::still();
        let plain = Logo::new(24.).look();
        assert_eq!((plain.tile, plain.ring, plain.dot), (still.tile, still.ring, still.dot));
    }
}
