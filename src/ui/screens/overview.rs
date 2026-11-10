//! Overview: the state of the book at a glance. The first screen built
//! entirely from `ui::kit`; it reflows on `Width` from one column up to a
//! full-width dashboard.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{header_link, status_detail_pill};
use crate::app::motion::Fresh;
use crate::app::{Route, VzApp};
use crate::charts::line::{LineChart, Series};
use crate::domain::ledger::DisplayStatus;
use crate::domain::money::{Cents, percent_label};
use crate::domain::periods::{Period, interest_in};
use crate::domain::portfolio::{CapitalKind, EntryKind, HistoryPoint, Summary};
use crate::i18n::{count, t};
use crate::services::hero_layout::{self, Figure as HeroFig, Spot};
use crate::services::overview_layout::{Block, Figure};
use crate::services::settings::ChartRange;
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::{avatar, empty_state, icon, kbd, kv, sel};
use crate::ui::kit::{
    Button, ButtonSize, Card, CountUp, Dropdown, DropdownItem, Fab, FabItem, FigureTone, GlassFigure, Glide, ListRow, Segmented, Stat,
    StatCards,
};
use crate::ui::theme::{Palette, Typeset as _, Width, palette, space, tabular, text};
use crate::utils::dates::short_month_year;

/// A list that stands on the page: a heading (with an optional count pill
/// and one action on the right) over its rows.
fn section(p: &Palette, title: &str, pill: Option<String>, action: impl IntoElement, body: AnyElement) -> Div {
    v_flex()
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .gap(px(space::M))
        .child(
            h_flex()
                .min_h(px(32.))
                .gap(px(10.))
                .child(div().typeset(text::HEADING).text_color(p.text).child(sel(title.to_string())))
                .children(pill.map(|label| {
                    div()
                        .px(px(7.))
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .rounded(px(5.))
                        .bg(p.accent_tint)
                        .typeset(text::CAPTION)
                        .font_weight(FontWeight::MEDIUM)
                        .font_features(tabular())
                        .text_color(p.accent_text)
                        .child(label)
                }))
                .child(div().flex_1())
                .child(action),
        )
        .child(body)
}

/// "3 loans" for the count pill beside a section heading.
fn count_label(n: usize) -> String {
    count(n, "loan", "loans", "préstamo", "préstamos")
}

/// Which screen's portfolio chart is being drawn; each keeps its own hover.
#[derive(Clone, Copy)]
pub(crate) enum ChartSlot {
    Overview,
    Analytics,
}

impl VzApp {
    pub(crate) fn render_overview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let w = Width::of(self.content_width(window));
        let gap = px(w.gap());
        let s = self.summary_now();
        let side_by_side = w >= Width::Wide;
        let editing = self.customizing;
        let glide = self.motion.gliding();
        // The figures sit in the hero above (hero_figures); the panel has the rest.
        let blocks: Vec<Block> = self.overview_blocks().into_iter().filter(|b| *b != Block::Figures).collect();
        let rows = self.settings().overview_layout.rows(&blocks);

        // One block, framed for arranging when Customize is on. `beside`:
        // it shares its row with another block.
        let block = |this: &Self, b: Block, beside: bool, cx: &mut Context<Self>| -> AnyElement {
            let el = match b {
                Block::Figures => this.kpi_cards(&p, &s, w, cx),
                Block::Growth => this.growth_card(&p, w, cx),
                Block::Money => this.allocation_card(&p, &s, beside, cx),
                Block::Attention => this.attention_list(&p, cx),
                Block::Recent => this.recent_payments_list(&p, cx),
                Block::Reminders => this.reminders_list(&p, cx),
                Block::Today => this.today_card(&p, cx),
                Block::Status => this.status_card(&p, cx),
            };
            if editing { this.block_frame(&p, b, el, cx) } else { el }
        };
        // How a block shares a row with `partner`: the chart takes twice
        // the money card's share, the money card stays readable, and
        // anything else splits the row evenly.
        let cell = |b: Block, partner: Block, el: AnyElement| -> AnyElement {
            let d = div().flex().flex_col().flex_basis(px(0.)).min_w_0();
            match b {
                Block::Growth if matches!(partner, Block::Money | Block::Today) => d.flex_grow(2.),
                Block::Today => d.flex_grow(1.).min_w(px(340.)),
                Block::Money => d.flex_grow(1.).min_w(px(360.)).max_w(px(540.)),
                _ => d.flex_grow(1.),
            }
            .child(el)
            .into_any_element()
        };
        let glides =
            |b: Block, el: AnyElement| Glide::new(SharedString::from(format!("ov-glide-{}", b.key())), el).armed(glide).into_any_element();
        let mut body = Vec::new();
        for row in rows {
            match row.as_slice() {
                [a, b] if side_by_side => {
                    // Two cards stretch to one height; lists keep their own.
                    let cards =
                        row.iter().all(|x| matches!(x, Block::Growth | Block::Money | Block::Figures | Block::Today | Block::Status));
                    let (ea, eb) = (block(self, *a, true, cx), block(self, *b, true, cx));
                    body.push(
                        h_flex()
                            .gap(gap)
                            .map(|d| if cards { d.items_stretch() } else { d.items_start() })
                            .child(glides(*a, cell(*a, *b, ea)))
                            .child(glides(*b, cell(*b, *a, eb)))
                            .into_any_element(),
                    );
                }
                blocks => {
                    for b in blocks {
                        let el = block(self, *b, false, cx);
                        body.push(glides(*b, el));
                    }
                }
            }
        }

        // Room at the page's end, clear of the + button.
        let end = (!editing).then(|| div().h(px(64.)));
        v_flex()
            .gap(gap)
            .when(editing, |d| d.child(self.customize_bar(&p, cx)))
            .children(self.pending_banner(&p, cx))
            .children(self.render_setup(&p, w, cx))
            .children(body)
            .when(editing, |d| d.children(self.hidden_tray(&p, cx)))
            .children(end)
            .into_any_element()
    }

    /// Every figure the hero can show, worked out once per frame: its words
    /// and how it reads (the web's definitions, src/screens/overview.tsx).
    fn figure_values(&self) -> HashMap<HeroFig, (String, FigureTone)> {
        let s = self.summary_now();
        let money = self.settings().money();
        let m = |c: Cents| money.format(c);
        let bad = |late: bool| if late { FigureTone::Bad } else { FigureTone::Plain };
        // The gain over the last six months, and how it compares.
        let then = self.today.checked_sub_months(chrono::Months::new(6)).unwrap_or(self.today);
        let past = self.snap.book.history(then, then).into_iter().next();
        let (gain6, pct) = match past {
            Some(h) => {
                let g = s.gain() - (h.portfolio_value - h.net_invested);
                (g, (h.net_invested > 0).then(|| percent_label(g, h.net_invested).unwrap_or_default()))
            }
            None => (s.gain(), None),
        };
        let gain_text = match pct {
            Some(pct) => format!("{} · {pct}", money.format_signed(gain6)),
            None => money.format_signed(gain6),
        };
        let gain_tone = match gain6.signum() {
            1 => FigureTone::Good,
            -1 => FigureTone::Bad,
            _ => FigureTone::Plain,
        };
        // Interest earned in the chosen period.
        let paid = || self.snap.payments.iter().filter(|x| x.reversed_at.is_none()).map(|x| (x.paid_on, x.interest));
        let earned = match self.settings().interest_period.spans(self.today) {
            Some((now, _)) => interest_in(paid(), now),
            None => s.interest_collected,
        };
        use chrono::Datelike as _;
        let month_start = self.today.with_day(1).unwrap_or(self.today);
        let collected: Cents = self
            .snap
            .payments
            .iter()
            .filter(|x| x.reversed_at.is_none() && x.paid_on >= month_start && x.paid_on <= self.today)
            .map(|x| x.amount)
            .sum();
        // What falls due in the next seven days.
        let soon = self.today + chrono::Days::new(7);
        let due7: Cents = self
            .snap
            .book
            .loans
            .iter()
            .map(|l| l.delinquency(self.today))
            .filter(|d| d.next_due.is_some_and(|n| n >= self.today && n <= soon))
            .map(|d| d.next_due_amount)
            .sum();
        // A collector's payments waiting for an admin.
        let waiting: Cents = self.cloud_copy().map_or(0, |c| c.pending.iter().filter(|p| p.status == "waiting").map(|p| p.amount).sum());
        let plain = |v: String| (v, FigureTone::Plain);
        HashMap::from([
            (HeroFig::Portfolio, plain(m(s.portfolio_value))),
            (HeroFig::LentOut, plain(m(s.outstanding_principal))),
            (HeroFig::InterestEarned, plain(m(earned))),
            (HeroFig::AvailableCapital, plain(m(s.available_capital))),
            (HeroFig::AvailableInterest, plain(m(s.available_interest))),
            (HeroFig::Receivable, plain(m(s.interest_receivable + s.fees_receivable))),
            (HeroFig::Overdue, (m(s.overdue_amount), bad(s.overdue_amount > 0))),
            (HeroFig::CollectedMonth, plain(m(collected))),
            (HeroFig::ExpectedInterest, plain(m(s.expected_monthly_interest))),
            (HeroFig::DueNext7, plain(m(due7))),
            (HeroFig::Gain6m, (gain_text, gain_tone)),
            (HeroFig::ActiveLoans, plain(s.active_loans.to_string())),
            (HeroFig::OverdueLoans, (s.overdue_loans.to_string(), bad(s.overdue_loans > 0))),
            (HeroFig::ToConfirm, plain(m(waiting))),
        ])
    }

    /// Interest earned's period: a small calendar beside its label.
    fn period_pick(&self, cx: &mut Context<Self>) -> Dropdown {
        let period = self.settings().interest_period;
        let app = cx.entity().downgrade();
        Dropdown::new("interest-period")
            .icon(IconName::Calendar)
            .kind(crate::ui::kit::ButtonKind::Ghost)
            .small()
            .tooltip(Self::period_name(period))
            .rest_ink(palette(cx).figure().mute)
            .items(Period::ALL.iter().map(|x| {
                let (app, x) = (app.clone(), *x);
                DropdownItem::new(Self::period_name(x), move |_, w, cx| {
                    let _ = app.update(cx, |this, cx| this.set_interest_period(x, w, cx));
                })
                .selected(x == period)
            }))
    }

    /// One place while arranging: its big figure and two rows, each picked
    /// from what this person may see, or Nothing.
    fn figure_editor(&self, place: usize, layout: hero_layout::Layout, cx: &mut Context<Self>) -> GlassFigure {
        let p = palette(cx);
        let h = p.figure();
        let allowed = hero_layout::allowed(self.role());
        let app = cx.entity().downgrade();
        let slot = layout[place];
        let picker = |spot: Spot, now: Option<HeroFig>, label: &'static str| {
            let choices = std::iter::once(None).chain(allowed.iter().filter(|f| spot != Spot::Big || !f.row_only()).map(|f| Some(*f)));
            let name = |x: Option<HeroFig>| x.map_or(t("Nothing", "Nada"), HeroFig::label);
            let id = SharedString::from(format!("hero-pick-{place}-{spot:?}"));
            let menu = Dropdown::new(id).small().label(name(now)).items(choices.map(|x| {
                let app = app.clone();
                DropdownItem::new(name(x), move |_, w, cx| {
                    let _ = app.update(cx, |this, cx| this.set_hero_layout(hero_layout::set(layout, place, spot, x), w, cx));
                })
                .selected(x == now)
            }));
            v_flex().gap(px(4.)).child(div().typeset(text::CAPTION).text_color(h.mute).child(label)).child(menu)
        };
        let n = place + 1;
        GlassFigure::new(SharedString::from(format!("hero-edit-{place}"))).body(
            v_flex()
                .gap(px(10.))
                .child(div().typeset(text::LABEL).text_color(h.ink).child(tr!("Place {n}", "Lugar {n}")))
                .child(picker(Spot::Big, slot.big, t("Big figure", "Cifra grande")))
                .child(picker(Spot::Row(0), slot.rows[0], t("Row 1", "Fila 1")))
                .child(picker(Spot::Row(1), slot.rows[1], t("Row 2", "Fila 2"))),
        )
    }

    /// The hero's figures. Admins: the business's six figures (Cards Option
    /// B, picked 2026-11-07). Collectors and viewers: four cards as they
    /// chose them (saved to their account; the role decides what may be
    /// picked), each a picker while arranging.
    pub(crate) fn hero_figures(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.can(Need::Manage) {
            return self.hero_six(window, cx);
        }
        let layout = self.hero_layout();
        let editing = self.customizing;
        let cards: Vec<GlassFigure> = if editing {
            (0..layout.len()).map(|i| self.figure_editor(i, layout, cx)).collect()
        } else {
            let values = self.figure_values();
            let value = |f: HeroFig| values.get(&f).cloned().unwrap_or_else(|| (String::new(), FigureTone::Plain));
            hero_layout::shown(&layout)
                .into_iter()
                .enumerate()
                .map(|(i, (big, rows))| {
                    let mut card = GlassFigure::new(SharedString::from(format!("hero-fig-{i}")));
                    if let Some(f) = big {
                        card = card.big(f.label(), value(f).0);
                        if f == HeroFig::InterestEarned {
                            card = card.action(self.period_pick(cx));
                        }
                    }
                    for r in rows {
                        let (text, tone) = value(r);
                        card = card.row(r.label(), text, tone);
                    }
                    card
                })
                .collect()
        };
        // Four in a row on wide windows; two by two below that.
        // The hero has no sidebar: the window's own width decides.
        let wide = f32::from(window.viewport_size().width) >= 1100.;
        let gap = px(16.);
        let grid = if wide || cards.len() < 4 {
            h_flex().items_start().gap(gap).children(cards).into_any_element()
        } else {
            let mut it = cards.into_iter();
            let (a, b, c, d) = (it.next(), it.next(), it.next(), it.next());
            v_flex()
                .gap(gap)
                .child(h_flex().items_start().gap(gap).children(a).children(b))
                .child(h_flex().items_start().gap(gap).children(c).children(d))
                .into_any_element()
        };
        if !editing {
            return grid;
        }
        let ink = palette(cx).hero().mute;
        let role = self.role();
        v_flex()
            .gap(px(10.))
            .child(grid)
            .child(
                h_flex()
                    .gap(px(space::M))
                    .typeset(text::CAPTION)
                    .text_color(ink)
                    .child(t(
                        "Choose Nothing to leave a spot empty. A single row shows larger.",
                        "Elige Nada para dejar un espacio vacío. Una sola fila se ve más grande.",
                    ))
                    .child(
                        Button::new("hero-figures-reset")
                            .ghost()
                            .small()
                            .label(t("Reset figures", "Restablecer cifras"))
                            .rest_ink(ink)
                            .on_click(cx.listener(move |this, _, w, cx| this.set_hero_layout(hero_layout::default_for(role), w, cx))),
                    ),
            )
            .into_any_element()
    }

    /// The admins' six figures (Cards Option B): the business's totals as in
    /// the earlier versions (icon, label, amount, a line under it) on small
    /// cards, one row on wide windows and 3 + 3 below 1,100px. Amounts
    /// shrink to fit their card, never cut.
    fn hero_six(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let h = p.figure();
        let s = self.summary_now();
        let money = self.settings().money();
        // Under the launch splash the amounts wait at zero, then count up
        // once it opens (a new id starts the count).
        let covering = self.motion.covering;
        let viewport = f32::from(window.viewport_size().width);
        let wide = viewport >= 1100.;
        let gap = 12.;
        let per_row = if wide { 6. } else { 3. };
        // The figures' row is the panel's width (content plus its padding).
        let row_w = self.content_width(window);
        let inner = ((row_w - gap * (per_row - 1.)) / per_row - 38.).max(80.);
        // Inter's figures are about 0.6em wide: the size that fits the card.
        let fit = |text: &str| (inner / (text.chars().count().max(1) as f32 * 0.62)).clamp(14., 22.);
        let toned = |v: Cents| match v.signum() {
            1 => p.positive,
            -1 => p.negative,
            _ => h.mute,
        };
        // A card (the clean look): the label with its icon at the top right,
        // the amount, then up to two caption lines.
        let card = |id: &str,
                    glyph: IconName,
                    chip: (Hsla, Hsla),
                    label: &str,
                    value: Cents,
                    value_color: Hsla,
                    lines: Vec<(String, Hsla)>,
                    action: Option<AnyElement>| {
            let size = fit(&money.format(value));
            GlassFigure::new(SharedString::from(format!("hero-six-{id}"))).compact().body(
                v_flex()
                    .h(px(104.))
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .h(px(28.))
                            .child(
                                div()
                                    .flex_shrink(1.)
                                    .min_w_0()
                                    .truncate()
                                    .typeset(text::CAPTION)
                                    .text_color(h.mute)
                                    .child(label.to_string()),
                            )
                            .child({
                                let hint = six_hint(id);
                                div()
                                    .id(SharedString::from(format!("six-info-{id}")))
                                    .flex_none()
                                    .size(px(15.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(h.mute)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(9.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(h.mute)
                                    .cursor_default()
                                    .child("i")
                                    .tooltip(move |w, cx| Tooltip::new(hint).build(w, cx))
                            })
                            .child(div().flex_1())
                            .children(action)
                            .child(
                                div()
                                    .flex_none()
                                    .size(px(28.))
                                    .rounded(px(9.))
                                    .bg(chip.1)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(glyph, chip.0, 15.)),
                            ),
                    )
                    .child(
                        div()
                            .typeset(text::FIGURE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(size))
                            .font_features(tabular())
                            .whitespace_nowrap()
                            .text_color(value_color)
                            .child({
                                let money = money.clone();
                                CountUp::new(
                                    SharedString::from(format!("hero-six-n-{id}-{covering}")),
                                    if covering { 0 } else { value },
                                    move |c| money.format(c),
                                )
                                .color(value_color)
                            }),
                    )
                    .child(
                        v_flex()
                            .h(px(32.))
                            .justify_end()
                            .typeset(text::CAPTION)
                            .font_features(tabular())
                            .children(lines.into_iter().map(|(l, c)| div().text_color(c).child(sel(l)))),
                    ),
            )
        };
        let mute = h.mute;
        // 1. Total portfolio value: the gain on what was invested.
        let (gain, invested) = (s.gain(), s.net_invested());
        let portfolio_lines = if invested > 0 {
            let pct = percent_label(gain, invested).unwrap_or_default();
            let inv = money.format(invested);
            vec![
                (format!("{} ({}{pct})", money.format_signed(gain), if gain > 0 { "+" } else { "" }), toned(gain)),
                (tr!("on {inv} invested", "sobre {inv} invertido"), mute),
            ]
        } else {
            vec![(t("No capital recorded yet", "Aún no hay capital registrado").to_string(), mute)]
        };
        // 2. Available capital: its share of the portfolio.
        let pv = s.portfolio_value;
        let cash_line = match percent_label(s.available_capital, pv) {
            Some(pct) if pv > 0 => tr!("{pct} of portfolio", "{pct} de la cartera"),
            _ => t("Ready to lend", "Listo para prestar").to_string(),
        };
        // 3. Available interest: what expenses took from it.
        let interest_line = if s.expenses() > 0 {
            let spent = money.format(s.expenses());
            tr!("{spent} spent in expenses", "{spent} usado en gastos")
        } else {
            t("Interest and fees on hand", "Interés y cargos en mano").to_string()
        };
        // 4. Overdue amount: how many loans are behind.
        let late = s.overdue_amount > 0;
        let overdue_line = match s.overdue_loans {
            0 => t("Nothing past due", "Nada vencido").to_string(),
            1 => t("1 loan behind", "1 préstamo atrasado").to_string(),
            n => tr!("{n} loans behind", "{n} préstamos atrasados"),
        };
        // 5. Capital lent out: across how many loans.
        let lent_line = {
            let n = count(s.active_loans, "active loan", "active loans", "préstamo activo", "préstamos activos");
            tr!("across {n}", "en {n}")
        };
        // 6. Interest earned in the chosen period, against the one before.
        let period = self.settings().interest_period;
        let paid = || self.snap.payments.iter().filter(|x| x.reversed_at.is_none()).map(|x| (x.paid_on, x.interest));
        let (earned, earned_lines) = match period.spans(self.today) {
            None => (s.interest_collected, vec![(t("Received, all time", "Cobrado en total").to_string(), mute)]),
            Some((now, before)) => {
                let (a, b) = (interest_in(paid(), now), interest_in(paid(), before));
                let diff = a - b;
                let pct = (b > 0)
                    .then(|| percent_label(diff, b).unwrap_or_default())
                    .map(|x| format!(" ({}{x})", if diff > 0 { "+" } else { "" }));
                let versus = match period {
                    Period::Day => t("vs yesterday", "vs ayer"),
                    Period::Week => t("vs last week", "vs la semana pasada"),
                    Period::Month => t("vs last month", "vs el mes pasado"),
                    _ => t("vs last year", "vs el año pasado"),
                };
                (a, vec![(format!("{}{}", money.format_signed(diff), pct.unwrap_or_default()), toned(diff)), (versus.to_string(), mute)])
            }
        };
        let cards = vec![
            card(
                "portfolio",
                IconName::Wallet,
                (p.accent_text, p.accent_tint),
                t("Total portfolio value", "Valor total de la cartera"),
                pv,
                h.ink,
                portfolio_lines,
                None,
            ),
            card(
                "capital",
                IconName::Landmark,
                (p.accent_text, p.accent_tint),
                t("Available capital", "Capital disponible"),
                s.available_capital,
                h.ink,
                vec![(cash_line, mute)],
                None,
            ),
            card(
                "lent",
                IconName::HandCoins,
                (p.info, p.info_tint),
                t("Capital lent out", "Capital prestado"),
                s.outstanding_principal,
                h.ink,
                vec![(lent_line, mute)],
                None,
            ),
            card(
                "interest",
                IconName::Coins,
                (p.positive, p.positive_tint),
                t("Available interest", "Interés disponible"),
                s.available_interest,
                h.ink,
                vec![(interest_line, mute)],
                None,
            ),
            card(
                "earned",
                IconName::TrendingUp,
                (p.positive, p.positive_tint),
                t("Interest earned", "Interés cobrado"),
                earned,
                h.ink,
                earned_lines,
                Some(self.period_pick(cx).into_any_element()),
            ),
            card(
                "overdue",
                IconName::CircleAlert,
                if late { (p.negative, p.negative_tint) } else { (p.muted, p.neutral_tint) },
                t("Overdue amount", "Monto atrasado"),
                s.overdue_amount,
                if late { h.bad } else { h.ink },
                vec![(overdue_line, if late { h.bad } else { mute })],
                None,
            ),
        ];
        let gap = px(gap);
        if wide {
            h_flex().gap(gap).children(cards).into_any_element()
        } else {
            let mut it = cards.into_iter();
            let first: Vec<_> = it.by_ref().take(3).collect();
            v_flex().gap(px(16.)).child(h_flex().gap(gap).children(first)).child(h_flex().gap(gap).children(it)).into_any_element()
        }
    }

    /// Arrange: a small quiet button (sliders and a short word) at the
    /// hero's top right, over the figures (picked 2026-11-06), that opens
    /// the Overview's arranging. `ink`: its words at rest on the picture.
    pub(crate) fn arrange_button(&self, ink: Hsla, cx: &mut Context<Self>) -> AnyElement {
        Button::new("ov-arrange")
            .ghost()
            .small()
            .icon(IconName::SlidersHorizontal)
            .label(t("Arrange", "Organizar"))
            .rest_ink(ink)
            .tooltip(t("Arrange this Overview", "Organizar este Resumen"))
            .on_click(cx.listener(|this, _, _, cx| this.set_customizing(true, cx)))
            .into_any_element()
    }

    /// The "+" button: what the Overview's header offered before (Record
    /// payment nearest, New record, and for admins New loan and Add
    /// capital), with the same shortcuts. None for viewers.
    pub(crate) fn add_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.can(Need::Record) || self.customizing {
            return None;
        }
        let mut fab = Fab::new("add", t("Add", "Agregar"))
            .start_open(self.add_open_at_start)
            .item(
                FabItem::new(
                    t("Record payment", "Registrar pago"),
                    IconName::Banknote,
                    cx.listener(|this, _, w, cx| this.open_payment_form(None, w, cx)),
                )
                .hint("Ctrl+P")
                .primary(),
            )
            .item(
                FabItem::new(
                    t("New record", "Nuevo registro"),
                    IconName::ReceiptText,
                    cx.listener(|this, _, w, cx| this.open_entry_form(EntryKind::Expense, None, w, cx)),
                )
                .hint("Ctrl+E"),
            );
        if self.can(Need::Manage) {
            fab = fab
                .item(
                    FabItem::new(
                        t("New loan", "Nuevo préstamo"),
                        IconName::HandCoins,
                        cx.listener(|this, _, w, cx| this.open_loan_form(None, w, cx)),
                    )
                    .hint("Ctrl+L"),
                )
                .item(FabItem::new(
                    t("Add capital", "Agregar capital"),
                    IconName::Landmark,
                    cx.listener(|this, _, w, cx| this.open_capital_form(CapitalKind::Contribution, w, cx)),
                ));
        }
        Some(fab.into_any_element())
    }

    /// First-run checklist, shown until the first loan exists.
    fn render_setup(&self, p: &Palette, w: Width, cx: &mut Context<Self>) -> Option<AnyElement> {
        // The first steps are an admin's to take.
        if !self.snap.loans.is_empty() || !self.can(Need::Manage) {
            return None;
        }
        let has_capital = !self.snap.capital.is_empty();
        let has_client = !self.snap.clients.is_empty();
        let done = [has_capital, has_client, false].iter().filter(|d| **d).count();
        let step = |n: usize, done: bool, title: &str, body: &str, action: AnyElement| {
            h_flex()
                .flex_1()
                .flex_basis(px(0.))
                .min_w_0()
                .items_start()
                .gap(px(space::M))
                .p(px(space::L))
                .rounded(px(10.))
                .bg(p.raised)
                .child(
                    h_flex()
                        .flex_none()
                        .size(px(24.))
                        .rounded_full()
                        .justify_center()
                        .bg(if done { p.positive_tint } else { p.surface })
                        .border_1()
                        .border_color(if done { p.positive_tint } else { p.border })
                        .typeset(text::CAPTION)
                        .font_features(tabular())
                        .text_color(if done { p.positive } else { p.muted })
                        .child(if done {
                            icon(IconName::Check, p.positive, 14.).into_any_element()
                        } else {
                            n.to_string().into_any_element()
                        }),
                )
                .child(
                    v_flex()
                        .gap(px(4.))
                        .flex_1()
                        .min_w_0()
                        .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(title.to_string()))
                        .child(div().typeset(text::CAPTION).text_color(p.muted).child(body.to_string()))
                        .child(h_flex().pt(px(6.)).child(action)),
                )
        };
        let steps = [
            step(
                1,
                has_capital,
                t("Record your starting capital", "Registra tu capital inicial"),
                t("The money you have set aside to lend.", "El dinero que apartaste para prestar."),
                Button::new("setup-capital")
                    .small()
                    .label(t("Add capital", "Agregar capital"))
                    .on_click(cx.listener(|this, _, w, cx| this.open_capital_form(CapitalKind::Contribution, w, cx)))
                    .into_any_element(),
            ),
            step(
                2,
                has_client,
                t("Add a client", "Agrega un cliente"),
                t("Name and optional contact details.", "Nombre y datos de contacto opcionales."),
                Button::new("setup-client")
                    .small()
                    .label(t("New client", "Nuevo cliente"))
                    .on_click(cx.listener(|this, _, w, cx| this.open_client_form(None, w, cx)))
                    .into_any_element(),
            ),
            step(
                3,
                false,
                t("Create the first loan", "Crea el primer préstamo"),
                t("See what the client pays, and when, before you confirm.", "Mira qué paga el cliente, y cuándo, antes de confirmar."),
                Button::new("setup-loan")
                    .small()
                    .primary()
                    .label(t("New loan", "Nuevo préstamo"))
                    .on_click(cx.listener(|this, _, w, cx| this.open_loan_form(None, w, cx)))
                    .into_any_element(),
            ),
        ];
        Some(
            Card::new()
                .title(t("Set up your lending book", "Configura tu cartera"))
                .action(
                    div()
                        .typeset(text::CAPTION)
                        .text_color(p.muted)
                        .font_features(tabular())
                        .child(tr!("{done} of 3 done", "{done} de 3 listos")),
                )
                .child(
                    v_flex()
                        .px(px(space::XL))
                        .pb(px(space::XL))
                        .gap(px(space::L))
                        .child(div().text_color(p.muted).child(t(
                            "Three steps, and every figure on this screen comes from what you record.",
                            "Tres pasos, y cada cifra de esta pantalla sale de lo que registres.",
                        )))
                        .child(
                            div()
                                .flex()
                                .gap(px(space::M))
                                .when(w >= Width::Medium, |d| d.flex_row().items_stretch())
                                .when(w < Width::Medium, |d| d.flex_col())
                                .children(steps),
                        ),
                )
                .into_any_element(),
        )
    }

    /// The figure cards, in the person's order. While arranging, hidden
    /// ones show faded with their tools so they can come back.
    fn kpi_cards(&self, p: &Palette, s: &Summary, w: Width, cx: &mut Context<Self>) -> AnyElement {
        let layout = &self.settings().overview_layout;
        let editing = self.customizing;
        let allowed = self.overview_figures();
        let order: Vec<Figure> =
            layout.figures.iter().copied().filter(|f| allowed.contains(f) && (editing || layout.shows_figure(*f))).collect();
        if order.is_empty() {
            return div().into_any_element();
        }
        // Six full money figures need about 1,680px; below that, 3 + 3.
        // Figures are never truncated to make a row fit.
        let most = if !self.can(Need::Manage) {
            if w == Width::Compact { 2 } else { 3 }
        } else {
            match w {
                Width::Compact => 2,
                Width::Medium | Width::Wide => 3,
                Width::Ultra => 6,
            }
        };
        let stats: Vec<Stat> = order
            .iter()
            .map(|f| {
                let st = self.figure_stat(*f, p, s, cx);
                if !editing {
                    return st;
                }
                let hidden = !layout.shows_figure(*f);
                st.accessory(self.figure_tools(*f, cx)).when(hidden, |st| st.opacity(0.45))
            })
            .collect();
        let mut cards = StatCards::new(most.min(order.len()))
            .spacing(w.gap())
            .lifting("ov-figures")
            .keys(order.iter().map(|f| f.key()))
            .glide(self.motion.gliding())
            .stats(stats);
        if editing {
            let drags: Vec<_> = order.iter().map(|f| self.figure_drag(*f, cx)).collect();
            cards = cards.frame(move |i, card| match drags.get(i) {
                Some(drag) => drag(card),
                None => card,
            });
        }
        cards.into_any_element()
    }

    /// One figure card. Amounts count up when the Overview opens and roll
    /// to a new amount after a change.
    fn figure_stat(&self, f: Figure, p: &Palette, s: &Summary, cx: &mut Context<Self>) -> Stat {
        let money = self.settings().money();
        let counted = |st: Stat, value: Cents| {
            let money = money.clone();
            st.counting(SharedString::from(format!("ov-count-{}", f.key())), value, move |v| money.format(v))
        };
        let counted_n =
            |st: Stat, value: usize| st.counting(SharedString::from(format!("ov-count-{}", f.key())), value as i64, |v| v.to_string());
        match f {
            Figure::Overdue => {
                let overdue_sub = match s.overdue_loans {
                    0 => t("Nothing past due", "Nada vencido").to_string(),
                    1 => t("1 loan behind", "1 préstamo atrasado").to_string(),
                    n => tr!("{n} loans behind", "{n} préstamos atrasados"),
                };
                let late = s.overdue_amount > 0;
                let st = Stat::new(t("Overdue amount", "Monto atrasado"), money.format(s.overdue_amount))
                    .icon(IconName::CircleAlert)
                    .chip(if late { p.negative } else { p.muted }, if late { p.negative_tint } else { p.neutral_tint })
                    .when(late, |st| st.tone(p.negative))
                    .caption(sel(overdue_sub));
                counted(st, s.overdue_amount)
            }
            Figure::ActiveLoans => counted_n(
                Stat::new(t("Active loans", "Préstamos activos"), s.active_loans.to_string())
                    .icon(IconName::HandCoins)
                    .chip(p.accent_text, p.accent_tint),
                s.active_loans,
            ),
            Figure::DueSoon => counted_n(
                Stat::new(t("Due soon", "Por vencer"), s.due_soon_loans.to_string())
                    .icon(IconName::Calendar)
                    .chip(p.warning, p.warning_tint)
                    .caption(sel(t("Payments in the next few days", "Pagos en los próximos días"))),
                s.due_soon_loans,
            ),
            Figure::PortfolioValue => {
                let gain = s.gain();
                let invested = s.net_invested();
                let gain_sub: AnyElement = if invested > 0 {
                    let pct = percent_label(gain, invested).unwrap_or_default();
                    let c = match gain.signum() {
                        1 => p.positive,
                        -1 => p.negative,
                        _ => p.muted,
                    };
                    h_flex()
                        .gap(px(4.))
                        .child(div().text_color(c).font_features(tabular()).child(sel(format!(
                            "{} ({}{})",
                            money.format_signed(gain),
                            if gain > 0 { "+" } else { "" },
                            pct
                        ))))
                        .child(div().font_features(tabular()).min_w_0().truncate().child(sel({
                            let inv = money.format(invested);
                            tr!("on {inv} invested", "sobre {inv} invertido")
                        })))
                        .into_any_element()
                } else {
                    t("No capital recorded yet", "Aún no hay capital registrado").into_any_element()
                };
                let adjust = Button::new("kpi-adjust")
                    .ghost()
                    .size(ButtonSize::Mini)
                    .icon(IconName::Pencil)
                    .tooltip(t("Adjust balances", "Ajustar saldos"))
                    .on_click(cx.listener(|this, _, w, cx| this.open_adjust_form(w, cx)));
                let st = Stat::new(t("Total portfolio value", "Valor total de la cartera"), money.format(s.portfolio_value))
                    .icon(IconName::Wallet)
                    .chip(p.accent_text, p.accent_tint)
                    .when(self.can(Need::Manage), |s| s.accessory(adjust))
                    .caption(gain_sub);
                counted(st, s.portfolio_value)
            }
            Figure::LentOut => {
                let st = Stat::new(t("Capital lent out", "Capital prestado"), money.format(s.outstanding_principal))
                    .icon(IconName::HandCoins)
                    .chip(p.info, p.info_tint)
                    .caption(sel({
                        let n = count(s.active_loans, "active loan", "active loans", "préstamo activo", "préstamos activos");
                        tr!("across {n}", "en {n}")
                    }));
                counted(st, s.outstanding_principal)
            }
            Figure::InterestEarned => {
                let period = self.settings().interest_period;
                let spans = period.spans(self.today);
                let paid = || self.snap.payments.iter().filter(|x| x.reversed_at.is_none()).map(|x| (x.paid_on, x.interest));
                let (value, caption): (Cents, AnyElement) = match spans {
                    None => {
                        let sub = if s.fees_collected > 0 {
                            let fees = money.format(s.fees_collected);
                            tr!("plus {fees} in fees", "más {fees} en cargos")
                        } else {
                            t("Received, all time", "Cobrado en total").to_string()
                        };
                        (s.interest_collected, sel(sub).into_any_element())
                    }
                    Some((now, before)) => {
                        let (a, b) = (interest_in(paid(), now), interest_in(paid(), before));
                        let diff = a - b;
                        let pct = (b > 0)
                            .then(|| percent_label(diff, b).unwrap_or_default())
                            .map(|x| format!(" ({}{x})", if diff > 0 { "+" } else { "" }));
                        let versus = match period {
                            Period::Day => t("vs yesterday", "vs ayer"),
                            Period::Week => t("vs last week", "vs la semana pasada"),
                            Period::Month => t("vs last month", "vs el mes pasado"),
                            _ => t("vs last year", "vs el año pasado"),
                        };
                        let c = match diff.signum() {
                            1 => p.positive,
                            -1 => p.negative,
                            _ => p.muted,
                        };
                        let el = h_flex()
                            .gap(px(4.))
                            .child(div().text_color(c).font_features(tabular()).child(sel(format!(
                                "{}{}",
                                money.format_signed(diff),
                                pct.unwrap_or_default()
                            ))))
                            .child(div().min_w_0().truncate().child(versus))
                            .into_any_element();
                        (a, el)
                    }
                };
                let app = cx.entity().downgrade();
                let pick = Dropdown::new("interest-period")
                    .icon(IconName::Calendar)
                    .kind(crate::ui::kit::ButtonKind::Ghost)
                    .small()
                    .align_end()
                    .tooltip(t("Choose the period", "Elegir el período"))
                    .items(Period::ALL.iter().map(|x| {
                        let (app, x) = (app.clone(), *x);
                        DropdownItem::new(Self::period_name(x), move |_, w, cx| {
                            let _ = app.update(cx, |this, cx| this.update_settings(|s| s.interest_period = x, w, cx));
                        })
                        .selected(x == period)
                    }));
                let st = Stat::new(t("Interest earned", "Interés cobrado"), money.format(value))
                    .icon(IconName::TrendingUp)
                    .chip(p.positive, p.positive_tint)
                    .above(Self::period_name(period))
                    .when(!self.customizing, |st| st.accessory(pick))
                    .caption(caption);
                // A new period counts again from its own amount.
                let money = money.clone();
                st.counting(SharedString::from(format!("ov-count-{}-{}", f.key(), period.key())), value, move |v| money.format(v))
            }
            Figure::AvailableCapital => {
                let pv = s.portfolio_value;
                let cash_sub = match percent_label(s.available_capital, pv) {
                    Some(pct) if pv > 0 => tr!("{pct} of portfolio", "{pct} de la cartera"),
                    _ => t("Ready to lend", "Listo para prestar").to_string(),
                };
                let st = Stat::new(t("Available capital", "Capital disponible"), money.format(s.available_capital))
                    .icon(IconName::Landmark)
                    .chip(p.accent_text, p.accent_tint)
                    .caption(sel(cash_sub));
                counted(st, s.available_capital)
            }
            Figure::AvailableInterest => {
                let interest_sub = if s.expenses() > 0 {
                    let spent = money.format(s.expenses());
                    tr!("{spent} spent in expenses", "{spent} usado en gastos")
                } else {
                    t("Interest and fees on hand", "Interés y cargos en mano").to_string()
                };
                let st = Stat::new(t("Available interest", "Interés disponible"), money.format(s.available_interest))
                    .icon(IconName::Coins)
                    .chip(p.positive, p.positive_tint)
                    .caption(sel(interest_sub));
                counted(st, s.available_interest)
            }
        }
    }

    pub(crate) fn growth_chart(
        &self,
        p: &Palette,
        points: &[HistoryPoint],
        slot: ChartSlot,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (hover, id) = match slot {
            ChartSlot::Overview => (self.chart_hover, "growth"),
            ChartSlot::Analytics => (self.analytics_hover, "an-growth"),
        };
        let ds = self.settings().date_style;
        let span_days = match (points.first(), points.last()) {
            (Some(a), Some(b)) => (b.date - a.date).num_days(),
            _ => 0,
        };
        let labels = points
            .iter()
            .map(|pt| if span_days > 400 { short_month_year(pt.date) } else { ds.format_short(pt.date, self.today) }.into())
            .collect();
        let hover_labels = points.iter().map(|pt| ds.format(pt.date).into()).collect();
        let mut series = vec![Series {
            name: t("Portfolio value", "Valor de la cartera").into(),
            values: points.iter().map(|x| x.portfolio_value).collect(),
            color: p.accent,
            width: 2.0,
            dashed: false,
            fill: true,
        }];
        if self.settings().show_interest_line {
            series.push(Series {
                name: t("Capital invested", "Capital invertido").into(),
                values: points.iter().map(|x| x.net_invested).collect(),
                color: p.chart_reference,
                width: 1.5,
                dashed: true,
                fill: false,
            });
        }
        let chart = LineChart { labels, hover_labels, series, height, money: self.settings().money(), animated: true };
        let weak = cx.entity().downgrade();
        let handler = Rc::new(move |h: Option<usize>, _: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |app, cx| {
                match slot {
                    ChartSlot::Overview => app.chart_hover = h,
                    ChartSlot::Analytics => app.analytics_hover = h,
                }
                cx.notify();
            });
        });
        chart.render(SharedString::from(id.to_string()), p, hover, handler)
    }

    /// How a period is named on Interest earned.
    fn period_name(p: Period) -> &'static str {
        match p {
            Period::Day => t("Today", "Hoy"),
            Period::Week => t("This week", "Esta semana"),
            Period::Month => t("This month", "Este mes"),
            Period::Year => t("This year", "Este año"),
            Period::All => t("All time", "Todo el tiempo"),
        }
    }

    /// The chart range picker: a segmented control, or a dropdown when the
    /// card is too narrow for five segments beside its title.
    fn range_control(&self, w: Width, cx: &mut Context<Self>) -> AnyElement {
        let range = self.settings().chart_range;
        let app = cx.entity().downgrade();
        let pick = |r: ChartRange| {
            let app = app.clone();
            move |_: &ClickEvent, w: &mut Window, cx: &mut App| {
                let _ = app.update(cx, |this, cx| this.update_settings(|s| s.chart_range = r, w, cx));
            }
        };
        if w == Width::Compact {
            return Dropdown::new("range")
                .small()
                .label(range.label())
                .align_end()
                .items(ChartRange::ALL.iter().map(|r| DropdownItem::new(r.label(), pick(*r)).selected(*r == range)))
                .into_any_element();
        }
        ChartRange::ALL
            .iter()
            .fold(Segmented::new("range").cell_width(42.), |seg, r| seg.option(r.label(), *r == range, pick(*r)))
            .into_any_element()
    }

    fn growth_card(&self, p: &Palette, w: Width, cx: &mut Context<Self>) -> AnyElement {
        let money = self.settings().money();
        let range = self.settings().chart_range;
        let points = self.chart_points(range);

        let (gain_change, interest_change) = match (points.first(), points.last()) {
            (Some(a), Some(b)) => {
                ((b.portfolio_value - b.net_invested) - (a.portfolio_value - a.net_invested), b.interest_collected - a.interest_collected)
            }
            _ => (0, 0),
        };
        let period = match range {
            ChartRange::All => t("since you started", "desde el inicio").to_string(),
            ChartRange::OneMonth => t("in the last month", "en el último mes").to_string(),
            ChartRange::ThreeMonths => t("in the last 3 months", "en los últimos 3 meses").to_string(),
            ChartRange::SixMonths => t("in the last 6 months", "en los últimos 6 meses").to_string(),
            ChartRange::OneYear => t("in the last year", "en el último año").to_string(),
        };
        let gain_color = match gain_change.signum() {
            1 => p.positive,
            -1 => p.negative,
            _ => p.muted,
        };
        let legend = h_flex()
            .gap(px(space::L))
            .typeset(text::CAPTION)
            .text_color(p.muted)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(12.)).h(px(2.)).rounded_full().bg(p.accent))
                    .child(t("Portfolio value", "Valor de la cartera")),
            )
            .when(self.settings().show_interest_line, |d| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .child(
                            h_flex()
                                .gap(px(2.))
                                .child(div().w(px(4.)).h(px(1.5)).bg(p.chart_reference))
                                .child(div().w(px(4.)).h(px(1.5)).bg(p.chart_reference)),
                        )
                        .child(t("Capital invested", "Capital invertido")),
                )
            });
        let chart_height = match w {
            Width::Compact => 200.,
            Width::Medium => 232.,
            Width::Wide => 296.,
            Width::Ultra => 320.,
        };

        let body = if points.is_empty() {
            empty_state(
                p,
                IconName::ChartLine,
                t("No history yet", "Aún no hay historial"),
                t(
                    "Portfolio value is charted from your capital, loans and payments as soon as you record them.",
                    "El valor de la cartera se grafica con tu capital, préstamos y pagos en cuanto los registres.",
                ),
                None,
            )
            .into_any_element()
        } else {
            v_flex()
                .px(px(space::XL))
                .pb(px(space::L))
                .gap(px(space::M))
                .child(
                    h_flex()
                        .justify_between()
                        .flex_wrap()
                        .gap(px(6.))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap(px(4.))
                                .text_color(p.muted)
                                .child(t("Gain", "Ganancia"))
                                .child(div().text_color(gain_color).font_features(tabular()).child(sel(money.format_signed(gain_change))))
                                .child(format!("{period} ·"))
                                .child(div().font_features(tabular()).child(sel({
                                    let i = money.format(interest_change);
                                    tr!("{i} interest received", "{i} de interés cobrado")
                                }))),
                        )
                        .child(legend),
                )
                .child(self.growth_chart(p, &points, ChartSlot::Overview, chart_height, cx))
                .into_any_element()
        };
        Card::new()
            .flex_grow(2.)
            .flex_basis(px(0.))
            .min_w_0()
            .title(t("Portfolio growth", "Crecimiento de la cartera"))
            .action(self.range_control(w, cx))
            .child(body)
            .into_any_element()
    }

    fn allocation_card(&self, p: &Palette, s: &Summary, beside: bool, cx: &mut Context<Self>) -> AnyElement {
        let money = self.settings().money();
        let total = s.portfolio_value.max(0);
        let mut cash_c = p.accent;
        cash_c.a = 0.35;
        let mut interest_c = p.positive;
        interest_c.a = 0.6;
        let parts: [(&str, Cents, Hsla); 4] = [
            (t("Lent out", "Prestado"), s.outstanding_principal, p.accent),
            (t("Interest due", "Interés vencido"), s.interest_receivable, p.chart_secondary),
            (t("Available capital", "Capital disponible"), s.available_capital.max(0), cash_c),
            (t("Available interest", "Interés disponible"), s.available_interest.max(0), interest_c),
        ];
        let bar = crate::ui::kit::motion::fill_in(
            h_flex().h(px(10.)).rounded(px(3.)).overflow_hidden().gap(px(2.)).bg(p.neutral_tint).children(
                parts
                    .iter()
                    .filter(|(_, v, _)| *v > 0 && total > 0)
                    .map(|(_, v, c)| div().h_full().flex_basis(px(0.)).flex_grow(*v as f32 / total as f32).bg(*c)),
            ),
            "money-fill",
        );
        // Label left; share and amount right-aligned in their own columns
        // so figures line up down the list.
        let rows = parts.iter().map(|(label, v, c)| {
            h_flex()
                .min_h(px(36.))
                .gap(px(10.))
                .child(div().flex_none().size(px(8.)).rounded(px(2.)).bg(*c))
                .child(div().flex_1().min_w_0().truncate().text_color(p.muted).child(sel(label.to_string())))
                .child(
                    div()
                        .flex_none()
                        .min_w(px(52.))
                        .text_right()
                        .typeset(text::CAPTION)
                        .text_color(p.faint)
                        .font_features(tabular())
                        .child(sel(percent_label(*v, total).filter(|_| total > 0).unwrap_or_else(|| "—".into()))),
                )
                .child(
                    div()
                        .flex_none()
                        .min_w(px(116.))
                        .text_right()
                        .whitespace_nowrap()
                        .font_features(tabular())
                        .text_color(p.text)
                        .child(sel(money.format(*v))),
                )
        });
        Card::new()
            .when(beside, |d| d.flex_grow(1.).flex_basis(px(0.)).min_w(px(360.)).max_w(px(520.)))
            .title(t("Where your money is", "Dónde está tu dinero"))
            .child(
                v_flex()
                    .px(px(space::XL))
                    .pb(px(space::XL))
                    .gap(px(space::L))
                    .child(bar)
                    .child(v_flex().children(rows))
                    .child(div().h(px(1.)).bg(p.border))
                    .child(kv(p, t("Expected interest per month", "Interés esperado por mes"), sel(money.format(s.expected_monthly_interest))))
                    .when(s.defaulted_principal > 0, |d| {
                        d.child(
                            h_flex()
                                .justify_between()
                                .child(div().text_color(p.negative).child(t("Defaulted, excluded", "Incobrable, excluido")))
                                .child(div().text_color(p.negative).font_features(tabular()).child(sel(money.format(s.defaulted_principal + s.defaulted_interest)))),
                        )
                    })
                    .when(self.can(Need::Manage), |d| d.child(
                        h_flex()
                            .gap(px(space::S))
                            .flex_wrap()
                            .child(
                                Button::new("alloc-move")
                                    .small()
                                    .icon(IconName::ArrowRightLeft)
                                    .label(t("Move money", "Mover dinero"))
                                    .on_click(cx.listener(|this, _, w, cx| this.open_entry_form(EntryKind::Transfer, None, w, cx))),
                            )
                            .child(
                                Button::new("alloc-adjust")
                                    .small()
                                    .icon(IconName::SlidersHorizontal)
                                    .label(t("Adjust balances", "Ajustar saldos"))
                                    .on_click(cx.listener(|this, _, w, cx| this.open_adjust_form(w, cx))),
                            ),
                    ))
                    .child(div().typeset(text::CAPTION).line_height(px(17.)).text_color(p.faint).child(t(
                        "Portfolio value = available capital + available interest + capital lent out + interest already due but not yet collected. Defaulted balances are left out.",
                        "Valor de la cartera = capital disponible + interés disponible + capital prestado + interés ya vencido sin cobrar. Los saldos incobrables no se cuentan.",
                    ))),
            )
            .into_any_element()
    }

    fn recent_payments_list(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let money = self.settings().money();
        let ds = self.settings().date_style;
        let recent: Vec<_> = self.snap.payments.iter().rev().filter(|x| !x.is_reversed()).take(5).collect();
        let rows = recent.iter().enumerate().map(|(i, pay)| {
            let loan = self.snap.loan(pay.loan_id);
            let client_id = loan.map(|l| l.client_id).unwrap_or(0);
            // The receipt on WhatsApp, when the client has a number it can go to.
            let wa = self.wa_receipt_button(pay.id, cx);
            let name = self.snap.client_name(client_id).to_string();
            let loan_id = pay.loan_id;
            let split = if pay.fee > 0 {
                let (i, c, f) = (money.format(pay.interest), money.format(pay.principal), money.format(pay.fee));
                tr!("Interest {i} · Capital {c} · Fee {f}", "Interés {i} · Capital {c} · Cargo {f}")
            } else {
                let (i, c) = (money.format(pay.interest), money.format(pay.principal));
                tr!("Interest {i} · Capital {c}", "Interés {i} · Capital {c}")
            };
            ListRow::new(("recent", pay.id as u64))
                .boxed(true)
                .rise(i)
                .fresh(self.motion.is_fresh(Fresh::Payment(pay.id)))
                .leading(avatar(p, &name, client_id, 32.))
                .title(name)
                .subtitle({
                    let (when, id) = (ds.format_short(pay.paid_on, self.today), pay.loan_id);
                    tr!("{when} · Loan #{id}", "{when} · Préstamo #{id}")
                })
                .trailing(
                    v_flex()
                        .flex_none()
                        .items_end()
                        .child(
                            div()
                                .typeset(text::BODY_STRONG)
                                .font_features(tabular())
                                .text_color(p.positive)
                                .child(sel(format!("+{}", money.format(pay.amount)))),
                        )
                        .child(div().typeset(text::CAPTION).font_features(tabular()).text_color(p.muted).child(sel(split))),
                )
                .when_some(wa, |r, b| r.trailing(b))
                .on_click(cx.listener(move |this, _, w, cx| this.open_row(Route::Loan(loan_id), w, cx)))
        });
        let body = if recent.is_empty() {
            Card::new()
                .child(empty_state(
                    p,
                    IconName::Receipt,
                    t("No payments yet", "Aún no hay pagos"),
                    t(
                        "Payments you record show here with their split between interest and capital.",
                        "Los pagos que registres aparecen aquí con su división entre interés y capital.",
                    ),
                    None,
                ))
                .into_any_element()
        } else {
            v_flex().gap(px(space::S)).children(rows).into_any_element()
        };
        section(
            p,
            t("Recent payments", "Pagos recientes"),
            None,
            header_link(p, "recent-all", t("View all", "Ver todos"), Route::Payments, cx),
            body,
        )
        .into_any_element()
    }

    /// Today's WhatsApp reminders: clients whose installment is due
    /// tomorrow or today, or who are late, each with a button that opens
    /// WhatsApp with the message written. Sent ones say so until tomorrow.
    fn reminders_list(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        use crate::services::whatsapp::Due;
        let money = self.settings().money();
        let all = self.wa_reminders();
        let left = all.iter().filter(|r| !self.wa_sent(r)).count();
        let rows = all.iter().enumerate().map(|(i, r)| {
            let id = r.loan_id;
            let when = match r.due {
                Due::Tomorrow => tr!("Loan #{id} · due tomorrow", "Préstamo #{id} · vence mañana"),
                Due::Today => tr!("Loan #{id} · due today", "Préstamo #{id} · vence hoy"),
                Due::Late(1) => tr!("Loan #{id} · 1 day late", "Préstamo #{id} · 1 día de atraso"),
                Due::Late(n) => tr!("Loan #{id} · {n} days late", "Préstamo #{id} · {n} días de atraso"),
            };
            let late = matches!(r.due, Due::Late(_));
            let sent = self.wa_sent(r);
            let action: AnyElement =
                match (&r.phone, sent) {
                    (None, _) => div()
                        .flex_none()
                        .min_w(px(96.))
                        .text_right()
                        .typeset(text::CAPTION)
                        .text_color(p.faint)
                        .child(t("No phone", "Sin teléfono"))
                        .into_any_element(),
                    (Some(_), done) => {
                        let r = r.clone();
                        Button::new(("wa-send", id as u64))
                            .ghost()
                            .small()
                            .map(|b| {
                                if done {
                                    b.icon(IconName::CheckCheck)
                                } else {
                                    b.prefix(img(crate::ui::pictures::whatsapp()).size(px(15.)))
                                }
                            })
                            .label(if done { t("Sent", "Enviado") } else { t("WhatsApp", "WhatsApp") })
                            .when(done, |b| b.rest_ink(p.positive))
                            .tooltip(if done {
                                t("Send it again", "Enviarlo de nuevo")
                            } else {
                                t("Open WhatsApp with the reminder written", "Abrir WhatsApp con el recordatorio escrito")
                            })
                            .on_click(cx.listener(move |this, _, w, cx| {
                                cx.stop_propagation();
                                this.send_reminder(&r, w, cx)
                            }))
                            .into_any_element()
                    }
                };
            ListRow::new(("wa", id as u64))
                .boxed(true)
                .rise(i)
                .muted(sent)
                .leading(avatar(p, &r.client, r.client_id, 32.))
                .title(r.client.clone())
                .subtitle(when)
                .trailing(
                    div()
                        .flex_none()
                        .min_w(px(88.))
                        .text_right()
                        .whitespace_nowrap()
                        .typeset(text::BODY_STRONG)
                        .font_features(tabular())
                        .text_color(if late { p.negative } else { p.text })
                        .child(sel(money.format(r.amount))),
                )
                .trailing(action)
                .on_click(cx.listener(move |this, _, w, cx| this.open_row(Route::Loan(id), w, cx)))
        });
        let body = if all.is_empty() {
            Card::new()
                .child(empty_state(
                    p,
                    IconName::MessageCircle,
                    t("No reminders today", "Sin recordatorios hoy"),
                    t(
                        "Clients with an installment due tomorrow or today, or who are late, appear here to remind on WhatsApp.",
                        "Aquí aparecen los clientes con una cuota que vence mañana u hoy, o atrasados, para recordarles por WhatsApp.",
                    ),
                    None,
                ))
                .into_any_element()
        } else {
            v_flex().gap(px(space::S)).children(rows).into_any_element()
        };
        section(p, t("WhatsApp reminders", "Recordatorios por WhatsApp"), (left > 0).then(|| left.to_string()), div(), body)
            .into_any_element()
    }

    /// Loans that are late or due soon, each row its own small card under
    /// a section heading (trying out boxed rows here first).
    fn attention_list(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let money = self.settings().money();
        let ds = self.settings().date_style;
        let record = self.can(Need::Record);
        let mut items: Vec<_> = self
            .snap
            .book
            .loans
            .iter()
            .filter_map(|l| {
                let st = l.display_status(self.today);
                matches!(st, DisplayStatus::Overdue | DisplayStatus::DueSoon).then(|| (l, st, l.delinquency(self.today)))
            })
            .collect();
        items.sort_by(|a, b| a.1.cmp(&b.1).then(b.2.days_overdue.cmp(&a.2.days_overdue)).then(a.2.next_due.cmp(&b.2.next_due)));
        let count = items.len();
        let rows = items.iter().take(5).enumerate().map(|(i, (l, st, q))| {
            let name = self.snap.client_name(l.client_id).to_string();
            let loan_id = l.id;
            let id = l.id;
            let overdue = *st == DisplayStatus::Overdue;
            // The amount to collect now sits on the right, never truncated;
            // the subtitle only says which loan and since or until when.
            let due_now = if overdue { q.overdue_amount } else { q.next_due_amount };
            let when = match (overdue, if overdue { q.overdue_since } else { q.next_due }) {
                (true, Some(d)) => {
                    let when = ds.format_short(d, self.today);
                    tr!("Loan #{id} · past due since {when}", "Préstamo #{id} · vencido desde el {when}")
                }
                (false, Some(d)) => {
                    let when = ds.format_short(d, self.today);
                    tr!("Loan #{id} · due {when}", "Préstamo #{id} · vence el {when}")
                }
                _ => tr!("Loan #{id}", "Préstamo #{id}"),
            };
            ListRow::new(("attn", loan_id as u64))
                .boxed(true)
                .rise(i)
                .leading(avatar(p, &name, l.client_id, 32.))
                .title(name)
                .subtitle(when)
                .trailing(div().flex_none().child(status_detail_pill(p, *st, q, self.today).h(px(22.))))
                .trailing(
                    div()
                        .flex_none()
                        .min_w(px(88.))
                        .text_right()
                        .whitespace_nowrap()
                        .typeset(text::BODY_STRONG)
                        .font_features(tabular())
                        .text_color(if overdue { p.negative } else { p.text })
                        .child(sel(money.format(due_now))),
                )
                .when(record, |r| {
                    r.trailing(Button::new(("attn-pay", loan_id as u64)).ghost().small().label(t("Record", "Cobrar")).on_click(
                        cx.listener(move |this, _, w, cx| {
                            cx.stop_propagation();
                            this.open_payment_form(Some(loan_id), w, cx)
                        }),
                    ))
                })
                .on_click(cx.listener(move |this, _, w, cx| this.open_row(Route::Loan(loan_id), w, cx)))
        });
        let body = if count == 0 {
            Card::new()
                .child(empty_state(
                    p,
                    IconName::CircleCheck,
                    t("Everything is current", "Todo al día"),
                    if self.snap.book.loans.is_empty() {
                        t("Loans that are overdue or due soon will appear here.", "Aquí aparecerán los préstamos atrasados o por vencer.")
                    } else {
                        t(
                            "No loan is overdue or due in the next few days.",
                            "Ningún préstamo está atrasado ni por vencer en los próximos días.",
                        )
                    },
                    None,
                ))
                .into_any_element()
        } else {
            v_flex().gap(px(space::S)).children(rows).into_any_element()
        };
        section(
            p,
            t("Needs attention", "Requieren atención"),
            (count > 0).then(|| count_label(count)),
            header_link(p, "attn-all", t("View loans", "Ver préstamos"), Route::Loans, cx),
            body,
        )
        .child(
            h_flex()
                .gap(px(6.))
                .typeset(text::CAPTION)
                .text_color(p.faint)
                .child(t("Record a payment from anywhere with", "Registra un pago desde cualquier lugar con"))
                .child(kbd(p, "Ctrl P")),
        )
        .into_any_element()
    }
}

/// What each of the six cards means, in one sentence (the "i" beside its
/// name; the web's `hint` in six-figures.tsx).
fn six_hint(id: &str) -> &'static str {
    match id {
        "portfolio" => t(
            "Everything the business has: money on hand plus what borrowers still owe you in capital and interest due.",
            "Todo lo que tiene el negocio: el dinero en mano más lo que los clientes aún deben en capital e interés vencido.",
        ),
        "capital" => t(
            "Money you can lend now: the capital you put in, less what is lent out, plus capital paid back.",
            "Dinero que puedes prestar ahora: el capital que pusiste, menos lo prestado, más el capital devuelto.",
        ),
        "lent" => t("Capital your borrowers still owe on open loans.", "Capital que tus clientes aún deben en préstamos abiertos."),
        "interest" => t(
            "Interest and fees collected that you have not spent or moved to capital.",
            "Interés y cargos cobrados que no has gastado ni pasado a capital.",
        ),
        "earned" => t("Interest and fees collected in the period you pick.", "Interés y cargos cobrados en el período que elijas."),
        "overdue" => t(
            "What is past its due date and still unpaid, on all loans.",
            "Lo que pasó su fecha de pago y sigue sin pagar, en todos los préstamos.",
        ),
        _ => "",
    }
}
