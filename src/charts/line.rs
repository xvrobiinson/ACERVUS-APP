use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::nice_ticks;
use crate::domain::money::{Cents, MoneyFormat};
use crate::ui::kit::motion::ease_out;
use crate::ui::theme::{Elevation, Palette, RADIUS_CONTROL, Typeset as _, elevation, motion, tabular, text};

#[derive(Clone)]
pub struct Series {
    pub name: SharedString,
    pub values: Vec<Cents>,
    pub color: Hsla,
    pub width: f32,
    pub dashed: bool,
    /// Flat tint under the line (alpha is applied here).
    pub fill: bool,
}

#[derive(Clone)]
pub struct LineChart {
    pub labels: Vec<SharedString>,
    /// Long label per point for the hover readout.
    pub hover_labels: Vec<SharedString>,
    pub series: Vec<Series>,
    pub height: f32,
    pub money: MoneyFormat,
    /// The line draws itself in when the chart appears, moves into its new
    /// shape when the data changes, and the hover marker glides.
    pub animated: bool,
}

/// What a chart drew last, by its id, so new data can grow out of it.
struct Drawn {
    sig: u64,
    values: Vec<Vec<Cents>>,
    /// The data before the last change, while the line moves from it.
    before: Option<Vec<Vec<Cents>>>,
    changed_at: Instant,
}

thread_local! {
    static DRAWN: RefCell<HashMap<SharedString, Drawn>> = RefCell::new(HashMap::new());
}

/// How the line shows this frame: drawing in from the left (`reveal`, 0–1),
/// or moving from older values (`morph`, 0–1).
#[derive(Clone, Copy)]
struct Phase {
    reveal: f32,
    morph: f32,
}

/// `old` stretched or squeezed to `n` points, so two ranges can blend.
fn resample(old: &[Cents], n: usize) -> Vec<f32> {
    match (old.len(), n) {
        (0, _) => vec![0.0; n],
        (1, _) => vec![old[0] as f32; n],
        (m, _) => (0..n)
            .map(|i| {
                let x = if n <= 1 { 0.0 } else { i as f32 * (m - 1) as f32 / (n - 1) as f32 };
                let (a, f) = (x.floor() as usize, x.fract());
                let b = (a + 1).min(m - 1);
                old[a] as f32 + (old[b] - old[a]) as f32 * f
            })
            .collect(),
    }
}

const Y_LANE: f32 = 64.0;
const X_LANE: f32 = 26.0;
const TOP_PAD: f32 = 10.0;

pub type HoverHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

impl LineChart {
    fn point_count(&self) -> usize {
        self.series.iter().map(|s| s.values.len()).max().unwrap_or(0)
    }

    /// How this frame's line relates to the last one drawn under `id`: a
    /// fresh chart draws in, new data moves from the old line. Returns the
    /// data's signature, the old values while moving, and whether it draws in.
    fn phase_source(&self, id: &SharedString) -> (u64, Option<Rc<Vec<Vec<Cents>>>>, bool) {
        let values: Vec<Vec<Cents>> = self.series.iter().map(|s| s.values.clone()).collect();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        values.hash(&mut h);
        let sig = h.finish();
        DRAWN.with(|m| {
            let mut m = m.borrow_mut();
            let now = Instant::now();
            match m.get_mut(id) {
                Some(d) if d.sig != sig => {
                    // Blend only between the same lines (a series switched
                    // on or off draws in afresh).
                    let before = std::mem::replace(&mut d.values, values);
                    d.before = (before.len() == d.values.len()).then_some(before);
                    d.sig = sig;
                    d.changed_at = now;
                }
                Some(_) => {}
                None => {
                    m.insert(id.clone(), Drawn { sig, values, before: None, changed_at: now });
                }
            }
            let d = &m[id];
            // Still moving from the old line; otherwise (first time, or
            // shown again later) it draws in.
            let moving = d.changed_at.elapsed() < motion::MORPH;
            let before = d.before.clone().filter(|_| moving).map(Rc::new);
            let draws_in = before.is_none();
            (sig, before, draws_in)
        })
    }

    /// Renders the chart. `hover` is the highlighted point index, owned by the
    /// caller so the readout survives re-renders.
    pub fn render(self, id: impl Into<ElementId>, p: &Palette, hover: Option<usize>, on_hover: HoverHandler) -> AnyElement {
        let id: ElementId = id.into();
        let n = self.point_count();
        let all: Vec<Cents> = self.series.iter().flat_map(|s| s.values.iter().copied()).collect();
        let (min, max) = (all.iter().copied().min().unwrap_or(0), all.iter().copied().max().unwrap_or(0));
        let ticks = nice_ticks(min, max, 4, min >= 0 && max > 0 && min < max / 3);
        let (lo, hi) = (*ticks.first().unwrap_or(&0), *ticks.last().unwrap_or(&1));
        let span = (hi - lo).max(1) as f32;
        let plot_h = self.height - X_LANE - TOP_PAD;
        let frac_y = move |v: Cents| 1.0 - (v - lo) as f32 / span;

        let bounds_cell: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
        let series = Rc::new(self.series.clone());
        let grid = p.grid;
        let tick_fracs: Rc<Vec<f32>> = Rc::new(ticks.iter().map(|t| frac_y(*t)).collect());
        let hover_line = p.chart_reference;
        let surface = p.surface;

        // The line itself: drawn in, moved into shape, or at rest.
        let key = SharedString::from(id.to_string());
        let (sig, before, draws_in) = if self.animated { self.phase_source(&key) } else { (0, None, false) };
        let plot = {
            let bounds_cell = bounds_cell.clone();
            let series = series.clone();
            move |phase: Phase| {
                let bounds_cell = bounds_cell.clone();
                let (series, tick_fracs, before) = (series.clone(), tick_fracs.clone(), before.clone());
                canvas(
                    move |bounds, _, _| {
                        bounds_cell.set(Some(bounds));
                        bounds
                    },
                    move |bounds, _, window, _| {
                        let w = bounds.size.width;
                        let h = bounds.size.height;
                        let ox = bounds.origin.x;
                        let oy = bounds.origin.y;
                        let x_at = |i: usize| ox + if n <= 1 { w / 2.0 } else { w * (i as f32 / (n - 1) as f32) };
                        let y_at = |f: f32| oy + h * f;
                        for (i, f) in tick_fracs.iter().enumerate() {
                            let mut b = PathBuilder::stroke(px(1.));
                            b.move_to(point(ox, y_at(*f)));
                            b.line_to(point(ox + w, y_at(*f)));
                            if let Ok(path) = b.build() {
                                let mut c = grid;
                                if i == 0 {
                                    c.a = 1.0;
                                }
                                window.paint_path(path, c);
                            }
                        }
                        // Drawing in: only the part left of the pen shows.
                        let shown = Bounds::new(point(ox, oy - px(8.)), size(w * phase.reveal.clamp(0.0, 1.0), h + px(16.)));
                        window.with_content_mask(Some(ContentMask { bounds: shown }), |window| {
                            for (k, s) in series.iter().enumerate() {
                                if s.values.is_empty() {
                                    continue;
                                }
                                let old = before.as_ref().and_then(|b| b.get(k)).map(|o| resample(o, s.values.len()));
                                let pts: Vec<Point<Pixels>> = s
                                    .values
                                    .iter()
                                    .enumerate()
                                    .map(|(i, v)| {
                                        let v = match &old {
                                            Some(o) => o[i] + (*v as f32 - o[i]) * phase.morph,
                                            None => *v as f32,
                                        };
                                        point(x_at(i), y_at(1.0 - (v - lo as f32) / span))
                                    })
                                    .collect();
                                if s.fill && pts.len() > 1 {
                                    let mut b = PathBuilder::fill();
                                    b.move_to(point(pts[0].x, oy + h));
                                    for pt in &pts {
                                        b.line_to(*pt);
                                    }
                                    b.line_to(point(pts[pts.len() - 1].x, oy + h));
                                    b.close();
                                    if let Ok(path) = b.build() {
                                        let mut c = s.color;
                                        c.a = 0.07;
                                        window.paint_path(path, c);
                                    }
                                }
                                let mut b = PathBuilder::stroke(px(s.width));
                                if s.dashed {
                                    b = b.dash_array(&[px(4.), px(4.)]);
                                }
                                b.move_to(pts[0]);
                                if pts.len() == 1 {
                                    b.line_to(point(pts[0].x + px(0.5), pts[0].y));
                                }
                                for pt in pts.iter().skip(1) {
                                    b.line_to(*pt);
                                }
                                if let Ok(path) = b.build() {
                                    window.paint_path(path, s.color);
                                }
                            }
                        });
                    },
                )
                .size_full()
            }
        };
        let line_layer = if !self.animated {
            div().absolute().inset_0().child(plot(Phase { reveal: 1.0, morph: 1.0 })).into_any_element()
        } else {
            let key = ElementId::NamedChild(std::sync::Arc::new(id.clone()), sig.to_string().into());
            let animation = if draws_in {
                // Smooth start and finish, like a pen.
                Animation::new(motion::DRAW).with_easing(|t| if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 })
            } else {
                Animation::new(motion::MORPH).with_easing(ease_out)
            };
            div()
                .absolute()
                .inset_0()
                .with_animation(key, animation, move |d, t| {
                    let phase = if draws_in { Phase { reveal: t, morph: 1.0 } } else { Phase { reveal: 1.0, morph: t } };
                    d.child(plot(phase))
                })
                .into_any_element()
        };

        // The hover marker: a line down the chart and a dot on each series.
        // Animated charts let it glide from point to point.
        let marker = hover.filter(|i| *i < n).map(|i| {
            let series = series.clone();
            let draw = move |at: f32| {
                let series = series.clone();
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let w = bounds.size.width;
                        let h = bounds.size.height;
                        let x = bounds.origin.x + if n <= 1 { w / 2.0 } else { w * (at / (n - 1) as f32) };
                        let mut b = PathBuilder::stroke(px(1.));
                        b.move_to(point(x, bounds.origin.y));
                        b.line_to(point(x, bounds.origin.y + h));
                        if let Ok(path) = b.build() {
                            window.paint_path(path, hover_line);
                        }
                        for s in series.iter() {
                            if s.values.is_empty() {
                                continue;
                            }
                            // Between two points while gliding.
                            let a = (at.max(0.0).floor() as usize).min(s.values.len() - 1);
                            let b2 = (a + 1).min(s.values.len() - 1);
                            let v = s.values[a] as f32 + (s.values[b2] - s.values[a]) as f32 * at.max(0.0).fract();
                            let c = point(x, bounds.origin.y + h * (1.0 - (v - lo as f32) / span));
                            let r = px(4.);
                            window.paint_quad(
                                fill(Bounds::new(point(c.x - r, c.y - r), size(r * 2., r * 2.)), surface)
                                    .corner_radii(r)
                                    .border_widths(px(2.))
                                    .border_color(s.color),
                            );
                        }
                    },
                )
                .size_full()
            };
            let layer = div().absolute().inset_0();
            if self.animated {
                layer
                    .with_spring(scoped_id(&id, "marker"), SpringAnimation::new(motion::GLIDE).to(i as f32), move |d, at| d.child(draw(at)))
                    .into_any_element()
            } else {
                layer.child(draw(i as f32)).into_any_element()
            }
        });

        // Y tick labels, in their own lane.
        let y_labels = ticks.iter().map(|t| {
            let top = TOP_PAD + plot_h * frac_y(*t) - 8.0;
            div()
                .absolute()
                .left_0()
                .top(px(top))
                .w(px(Y_LANE - 12.0))
                .text_right()
                .typeset(text::CAPTION)
                .text_size(px(11.5))
                .whitespace_nowrap()
                .text_color(p.faint)
                .font_features(tabular())
                .child(self.money.format_compact(*t))
        });

        // X labels: about five, evenly spaced, first and last pulled inside.
        let label_count = n.min(6);
        let picks: Vec<usize> = if n == 0 {
            vec![]
        } else if label_count <= 1 {
            vec![0]
        } else {
            (0..label_count).map(|k| k * (n - 1) / (label_count - 1)).collect()
        };
        let x_labels = picks.into_iter().map(|i| {
            let frac = if n <= 1 { 0.5 } else { i as f32 / (n - 1) as f32 };
            let text = self.labels.get(i).cloned().unwrap_or_default();
            let el = div().absolute().bottom_0().w(px(90.)).typeset(text::CAPTION).text_color(p.faint).child(text);
            if frac <= 0.0 {
                el.left_0().text_left()
            } else if frac >= 1.0 {
                el.right_0().text_right()
            } else {
                el.left(relative(frac)).ml(px(-45.)).text_center()
            }
        });

        // Hover readout.
        let readout = hover.filter(|i| *i < n).map(|i| {
            let frac = if n <= 1 { 0.5 } else { i as f32 / (n - 1) as f32 };
            let rows = self.series.iter().filter_map(|s| s.values.get(i).map(|v| (s, *v))).map(|(s, v)| {
                h_flex()
                    .gap(px(8.))
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(div().size(px(7.)).rounded_full().bg(s.color))
                            .child(div().text_color(p.muted).child(s.name.clone())),
                    )
                    .child(div().font_features(tabular()).text_color(p.text).child(self.money.format(v)))
            });
            let el = v_flex()
                .absolute()
                .top(px(0.))
                .w(px(230.))
                .p(px(10.))
                .gap(px(4.))
                .bg(p.surface)
                .rounded(px(RADIUS_CONTROL))
                .border_1()
                .border_color(p.border)
                .shadow(elevation(p, Elevation::Floating))
                .typeset(text::CAPTION)
                .child(
                    // Caption at 500, as in pills.
                    div().text_color(p.text).font_weight(FontWeight::MEDIUM).child(self.hover_labels.get(i).cloned().unwrap_or_default()),
                )
                .children(rows);
            let right = frac > 0.6;
            let place = move |el: Div, frac: f32| {
                if right { el.right(relative(1.0 - frac)).mr(px(12.)) } else { el.left(relative(frac)).ml(px(12.)) }
            };
            if self.animated && n > 1 {
                // The box follows the marker.
                el.with_spring(scoped_id(&id, "readout"), SpringAnimation::new(motion::GLIDE).to(frac), place).into_any_element()
            } else {
                place(el, frac).into_any_element()
            }
        });

        let handler_move = on_hover.clone();
        let handler_leave = on_hover;
        let bounds_for_move = bounds_cell.clone();

        div()
            .relative()
            .w_full()
            .h(px(self.height))
            .children(y_labels)
            .child(
                div()
                    .id(id)
                    .absolute()
                    .left(px(Y_LANE))
                    .right_0()
                    .top(px(TOP_PAD))
                    .h(px(plot_h))
                    .child(line_layer)
                    .children(marker)
                    .children(readout)
                    .on_mouse_move(move |e: &MouseMoveEvent, window, cx| {
                        if let Some(b) = bounds_for_move.get()
                            && n > 0
                        {
                            let rel = ((e.position.x - b.origin.x) / b.size.width).clamp(0.0, 1.0);
                            let idx = (rel * (n.saturating_sub(1)) as f32).round() as usize;
                            if Some(idx) != hover {
                                handler_move(Some(idx), window, cx);
                            }
                        }
                    })
                    .on_hover(move |hovered, window, cx| {
                        if !*hovered {
                            handler_leave(None, window, cx);
                        }
                    }),
            )
            .child(div().absolute().left(px(Y_LANE)).right_0().bottom_0().h(px(X_LANE - 6.0)).children(x_labels))
            .into_any_element()
    }
}

fn scoped_id(id: &ElementId, part: &'static str) -> ElementId {
    ElementId::NamedChild(std::sync::Arc::new(id.clone()), part.into())
}
