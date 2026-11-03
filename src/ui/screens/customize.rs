//! Arranging the Overview (Customize): each block gets a bar to drag it by,
//! arrows to move it, Half/Full and Hide; each figure gets arrows and a
//! show/hide eye; hidden blocks wait in a tray at the bottom. Every change
//! is saved at once and the blocks slide to their new places.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::VzApp;
use crate::i18n::t;
use crate::services::overview_layout::{Block, Figure, OverviewLayout};
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::icon;
use crate::ui::kit::interact::faded;
use crate::ui::kit::{Button, ButtonSize, Segmented};
use crate::ui::theme::{Elevation, Palette, RADIUS_CARD, RADIUS_CONTROL, Typeset as _, elevation, palette, space, text};

/// What a drop target runs with the dragged thing.
type DropListener<T> = dyn Fn(&T, &mut Window, &mut App);

/// A block being dragged.
pub(crate) struct DragBlock(pub Block);

/// A figure card being dragged.
pub(crate) struct DragFigure(pub Figure);

/// What follows the pointer while dragging: the thing's name.
struct DragGhost(SharedString);

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        h_flex()
            .h(px(32.))
            .px(px(space::M))
            .gap(px(space::S))
            .rounded(px(RADIUS_CONTROL))
            .bg(p.surface)
            .border_1()
            .border_color(p.accent)
            .shadow(elevation(&p, Elevation::Floating))
            .typeset(text::LABEL)
            .text_color(p.text)
            .child(icon(IconName::GripVertical, p.faint, 14.))
            .child(self.0.clone())
    }
}

fn figure_label(f: Figure) -> &'static str {
    match f {
        Figure::PortfolioValue => t("Total portfolio value", "Valor total de la cartera"),
        Figure::LentOut => t("Capital lent out", "Capital prestado"),
        Figure::InterestEarned => t("Interest earned", "Interés cobrado"),
        Figure::AvailableCapital => t("Available capital", "Capital disponible"),
        Figure::AvailableInterest => t("Available interest", "Interés disponible"),
        Figure::ActiveLoans => t("Active loans", "Préstamos activos"),
        Figure::DueSoon => t("Due soon", "Por vencer"),
        Figure::Overdue => t("Overdue amount", "Monto atrasado"),
    }
}

impl VzApp {
    /// The blocks this person's Overview can show (the business's totals
    /// are for admins).
    pub(crate) fn overview_blocks(&self) -> Vec<Block> {
        let manage = self.can(Need::Manage);
        Block::ALL.into_iter().filter(|b| manage || !matches!(b, Block::Growth | Block::Money)).collect()
    }

    /// The figures this person's Overview can show.
    pub(crate) fn overview_figures(&self) -> Vec<Figure> {
        let manage = self.can(Need::Manage);
        Figure::ALL
            .into_iter()
            .filter(|f| match f {
                Figure::Overdue => true,
                Figure::ActiveLoans | Figure::DueSoon => !manage,
                _ => manage,
            })
            .collect()
    }

    /// Changes the Overview's arrangement, saves it, and lets the blocks
    /// slide to their new places.
    pub(crate) fn edit_layout(&mut self, f: impl FnOnce(&mut OverviewLayout), window: &mut Window, cx: &mut Context<Self>) {
        self.arm_glide();
        self.update_settings(|s| f(&mut s.overview_layout), window, cx);
    }

    pub(crate) fn set_customizing(&mut self, on: bool, cx: &mut Context<Self>) {
        self.arm_glide();
        self.customizing = on;
        cx.notify();
    }

    /// The bar over the Overview while arranging it.
    pub(crate) fn customize_bar(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .flex_wrap()
            .gap(px(space::M))
            .px(px(space::L))
            .py(px(space::M))
            .rounded(px(RADIUS_CARD))
            .bg(p.accent_tint)
            .text_color(p.accent_text)
            .child(icon(IconName::SlidersHorizontal, p.accent_text, 16.))
            .child(
                div().flex_1().min_w(px(220.)).child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(div().typeset(text::BODY_STRONG).child(t("Arranging your Overview.", "Organizando tu Resumen.")))
                        .child(t(
                            "Drag a block by its bar, or use the buttons on each one.",
                            "Arrastra un bloque por su barra o usa los botones de cada uno.",
                        )),
                ),
            )
            .child(
                Button::new("ov-layout-reset")
                    .label(t("Reset", "Restablecer"))
                    .tooltip(t("Put everything back the way the app ships", "Volver a como viene la app"))
                    .on_click(cx.listener(|this, _, w, cx| this.edit_layout(|l| *l = OverviewLayout::default(), w, cx))),
            )
            .child(
                Button::new("ov-layout-done")
                    .primary()
                    .icon(IconName::Check)
                    .label(t("Done", "Listo"))
                    .on_click(cx.listener(|this, _, _, cx| this.set_customizing(false, cx))),
            )
            .into_any_element()
    }

    /// A block while arranging: a dashed frame with a bar on top to drag it
    /// by, arrows, Half/Full and Hide. Another block dropped on it takes its
    /// place.
    pub(crate) fn block_frame(&self, p: &Palette, b: Block, content: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let layout = &self.settings().overview_layout;
        let allowed = self.overview_blocks();
        let shown: Vec<Block> = layout.blocks.iter().copied().filter(|x| allowed.contains(x) && layout.shows(*x)).collect();
        let at = shown.iter().position(|x| *x == b).unwrap_or(0);
        let label = b.label();
        let key = b.key();
        let half = layout.is_half(b);
        let (up_allowed, down_allowed) = (allowed.clone(), allowed.clone());
        let mini = |id: (&'static str, &'static str), icon_name: IconName, tip: String| {
            Button::new(SharedString::from(format!("{}-{}", id.0, id.1))).ghost().size(ButtonSize::Mini).icon(icon_name).tooltip(tip)
        };
        let accent = p.accent;
        let bar = h_flex()
            .id(SharedString::from(format!("ov-bar-{key}")))
            .gap(px(4.))
            .h(px(32.))
            .pl(px(space::S))
            .pr(px(4.))
            .rounded(px(RADIUS_CONTROL))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .cursor_grab()
            .on_drag(DragBlock(b), move |_, _, _, cx| cx.new(|_| DragGhost(label.into())))
            .child(icon(IconName::GripVertical, p.faint, 14.))
            .child(div().flex_1().min_w_0().truncate().typeset(text::LABEL).text_color(p.text).child(label))
            .child(
                mini(("ov-up", key), IconName::ArrowUp, tr!("Move {label} up", "Subir {label}"))
                    .disabled(at == 0)
                    .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.nudge(b, -1, &up_allowed), w, cx))),
            )
            .child(
                mini(("ov-down", key), IconName::ArrowDown, tr!("Move {label} down", "Bajar {label}"))
                    .disabled(at + 1 >= shown.len())
                    .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.nudge(b, 1, &down_allowed), w, cx))),
            )
            .when(b.sizable(), |d| {
                d.child(
                    Segmented::pick(
                        SharedString::from(format!("ov-size-{key}")),
                        [(true, t("Half", "Mitad")), (false, t("Full", "Completo"))],
                        half,
                        cx,
                        move |this, half, w, cx| this.edit_layout(|l| l.set_half(b, half), w, cx),
                    )
                    .cell_width(52.),
                )
            })
            .child(
                mini(("ov-hide", key), IconName::EyeOff, tr!("Hide {label}", "Ocultar {label}"))
                    .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.set_shown(b, false), w, cx))),
            );
        div()
            .id(SharedString::from(format!("ov-frame-{key}")))
            .flex()
            .flex_col()
            .gap(px(space::S))
            .p(px(space::S))
            .rounded(px(RADIUS_CARD + space::S))
            .border_1()
            .border_dashed()
            .border_color(faded(accent, 0.55))
            .drag_over::<DragBlock>(move |s, _, _, _| s.border_color(accent).bg(faded(accent, 0.06)))
            .on_drop(cx.listener(move |this, d: &DragBlock, w, cx| {
                let moved = d.0;
                this.edit_layout(|l| l.drop_onto(moved, b), w, cx);
            }))
            .child(bar)
            .child(content)
            .into_any_element()
    }

    /// Hidden blocks, each a button that brings it back.
    pub(crate) fn hidden_tray(&self, p: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let layout = &self.settings().overview_layout;
        let hidden: Vec<Block> = self.overview_blocks().into_iter().filter(|b| !layout.shows(*b)).collect();
        if hidden.is_empty() {
            return None;
        }
        Some(
            h_flex()
                .flex_wrap()
                .gap(px(space::S))
                .p(px(space::M))
                .rounded(px(RADIUS_CARD))
                .border_1()
                .border_dashed()
                .border_color(p.control)
                .text_color(p.muted)
                .child(icon(IconName::EyeOff, p.faint, 14.))
                .child(t("Hidden from your Overview:", "Ocultos en tu Resumen:"))
                .children(hidden.into_iter().map(|b| {
                    Button::new(SharedString::from(format!("ov-show-{}", b.key())))
                        .small()
                        .icon(IconName::Plus)
                        .label(b.label())
                        .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.set_shown(b, true), w, cx)))
                }))
                .into_any_element(),
        )
    }

    /// The buttons on a figure card while arranging: earlier, later, and
    /// show or hide.
    pub(crate) fn figure_tools(&self, f: Figure, cx: &mut Context<Self>) -> AnyElement {
        let layout = &self.settings().overview_layout;
        let allowed = self.overview_figures();
        let order: Vec<Figure> = layout.figures.iter().copied().filter(|x| allowed.contains(x)).collect();
        let at = order.iter().position(|x| *x == f).unwrap_or(0);
        let label = figure_label(f);
        let shown = layout.shows_figure(f);
        let key = f.key();
        let (a1, a2) = (allowed.clone(), allowed);
        let mini = |id: &str, icon_name: IconName, tip: String| {
            Button::new(SharedString::from(format!("{id}-{key}"))).ghost().size(ButtonSize::Mini).icon(icon_name).tooltip(tip)
        };
        h_flex()
            .gap(px(2.))
            .child(
                mini("fig-left", IconName::ArrowLeft, tr!("Move {label} earlier", "Mover {label} antes"))
                    .disabled(at == 0)
                    .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.nudge_figure(f, -1, &a1), w, cx))),
            )
            .child(
                mini("fig-right", IconName::ArrowRight, tr!("Move {label} later", "Mover {label} después"))
                    .disabled(at + 1 >= order.len())
                    .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.nudge_figure(f, 1, &a2), w, cx))),
            )
            .child(
                mini(
                    "fig-eye",
                    if shown { IconName::EyeOff } else { IconName::Eye },
                    if shown { tr!("Hide {label}", "Ocultar {label}") } else { tr!("Show {label}", "Mostrar {label}") },
                )
                .on_click(cx.listener(move |this, _, w, cx| this.edit_layout(|l| l.set_figure_shown(f, !shown), w, cx))),
            )
            .into_any_element()
    }

    /// Drag and drop for a figure card while arranging, to apply to the
    /// card's frame.
    pub(crate) fn figure_drag(&self, f: Figure, cx: &mut Context<Self>) -> Rc<dyn Fn(Stateful<Div>) -> Stateful<Div>> {
        let accent = palette(cx).accent;
        let label = figure_label(f);
        let dropped: Rc<DropListener<DragFigure>> = Rc::new(cx.listener(move |this, d: &DragFigure, w, cx| {
            let moved = d.0;
            this.edit_layout(|l| l.drop_figure_onto(moved, f), w, cx);
        }));
        Rc::new(move |card: Stateful<Div>| {
            let dropped = dropped.clone();
            card.cursor_grab()
                .on_drag(DragFigure(f), move |_, _, _, cx| cx.new(|_| DragGhost(label.into())))
                .drag_over::<DragFigure>(move |s, _, _, _| s.bg(faded(accent, 0.08)).rounded(px(RADIUS_CARD)))
                .on_drop(move |d: &DragFigure, w, cx| dropped(d, w, cx))
        })
    }
}

#[cfg(test)]
mod tests {
    use core::prelude::v1::test;

    use ::gpui;
    use gpui_kit::TestAppContext;

    use crate::services::Store;
    use crate::services::overview_layout::{Block, OverviewLayout};

    /// Arranging draws, a change is saved with the settings, and Reset
    /// puts the shipped layout back.
    #[gpui::test]
    fn arranging_saves_and_resets(cx: &mut TestAppContext) {
        let (app, cx) = crate::ui::forms::test_app(cx, Store::in_memory());
        cx.update(|window, cx| {
            app.update(cx, |a, cx| {
                a.set_customizing(true, cx);
                a.edit_layout(|l| l.set_shown(Block::Money, false), window, cx);
                a.edit_layout(|l| l.drop_onto(Block::Recent, Block::Figures), window, cx);
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |a, cx| {
                let saved = a.store.snapshot().unwrap().settings.overview_layout;
                assert!(!saved.shows(Block::Money));
                assert_eq!(saved.blocks[0], Block::Recent);
                a.edit_layout(|l| *l = OverviewLayout::default(), window, cx);
                a.set_customizing(false, cx);
            })
        });
        cx.run_until_parked();
        app.update(cx, |a, _| assert_eq!(a.settings().overview_layout, OverviewLayout::default()));
    }
}
