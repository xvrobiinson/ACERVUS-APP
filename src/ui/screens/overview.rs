//! Overview: the state of the book at a glance. The first screen built
//! entirely from `ui::kit`; it reflows on `Width` from one column up to a
//! full-width dashboard.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{header_link, status_detail_pill};
use crate::app::motion::Fresh;
use crate::app::{Route, VzApp};
use crate::charts::line::{LineChart, Series};
use crate::domain::ledger::DisplayStatus;
use crate::domain::money::{Cents, percent_label};
use crate::domain::portfolio::{CapitalKind, EntryKind, HistoryPoint, Summary};
use crate::i18n::{count, t};
use crate::services::overview_layout::{Block, Figure};
use crate::services::settings::ChartRange;
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::{avatar, empty_state, icon, kbd, kv, page_header, sel};
use crate::ui::kit::{Button, ButtonSize, Card, Dropdown, DropdownItem, Glide, ListRow, Segmented, Stat, StatCards};
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

/// The Overview title: a greeting for the time of day.
fn greeting() -> &'static str {
    use chrono::Timelike as _;
    match chrono::Local::now().hour() {
        5..=11 => t("Good morning", "Buenos días"),
        12..=17 => t("Good afternoon", "Buenas tardes"),
        _ => t("Good evening", "Buenas noches"),
    }
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
        let rows = self.settings().overview_layout.rows(&self.overview_blocks());

        // One block, framed for arranging when Customize is on. `beside`:
        // it shares its row with another block.
        let block = |this: &Self, b: Block, beside: bool, cx: &mut Context<Self>| -> AnyElement {
            let el = match b {
                Block::Figures => this.kpi_cards(&p, &s, w, cx),
                Block::Growth => this.growth_card(&p, w, cx),
                Block::Money => this.allocation_card(&p, &s, beside, cx),
                Block::Attention => this.attention_list(&p, cx),
                Block::Recent => this.recent_payments_list(&p, cx),
            };
            if editing { this.block_frame(&p, b, el, cx) } else { el }
        };
        // How a block shares a row with `partner`: the chart takes twice
        // the money card's share, the money card stays readable, and
        // anything else splits the row evenly.
        let cell = |b: Block, partner: Block, el: AnyElement| -> AnyElement {
            let d = div().flex().flex_col().flex_basis(px(0.)).min_w_0();
            match b {
                Block::Growth if partner == Block::Money => d.flex_grow(2.),
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
                    let cards = row.iter().all(|x| matches!(x, Block::Growth | Block::Money | Block::Figures));
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

        v_flex()
            .gap(gap)
            .child(
                page_header(&p, greeting(), Some(self.finding(&p, &s)), if editing { Vec::new() } else { self.header_actions(w, cx) })
                    .pb_0(),
            )
            .when(editing, |d| d.child(self.customize_bar(&p, cx)))
            .children(self.pending_banner(&p, cx))
            .children(self.render_setup(&p, w, cx))
            .children(body)
            .when(editing, |d| d.children(self.hidden_tray(&p, cx)))
            .into_any_element()
    }

    /// The one-line finding under the title.
    fn finding(&self, p: &Palette, s: &Summary) -> AnyElement {
        let money = self.settings().money();
        if s.overdue_loans > 0 {
            h_flex()
                .flex_wrap()
                .gap(px(4.))
                .child(sel({
                    let n = s.overdue_loans;
                    if n == 1 {
                        t("1 loan overdue ·", "1 préstamo atrasado ·").to_string()
                    } else {
                        tr!("{n} loans overdue ·", "{n} préstamos atrasados ·")
                    }
                }))
                .child(div().text_color(p.negative).font_features(tabular()).child(sel({
                    let amount = money.format(s.overdue_amount);
                    tr!("{amount} past due", "{amount} vencido")
                })))
                .into_any_element()
        } else if s.due_soon_loans > 0 {
            sel({
                let n = count(s.due_soon_loans, "payment", "payments", "pago", "pagos");
                tr!("{n} due in the next few days", "{n} por vencer en los próximos días")
            })
            .into_any_element()
        } else if s.active_loans > 0 {
            sel({
                let n = count(s.active_loans, "active loan", "active loans", "préstamo activo", "préstamos activos");
                tr!("All current: {n}", "Todo al día: {n}")
            })
            .into_any_element()
        } else {
            t("Your lending portfolio at a glance", "Tu cartera de préstamos de un vistazo").into_any_element()
        }
    }

    /// Page actions. Wide windows show all three; narrower ones fold the
    /// secondary actions into a "More" menu so the primary never wraps.
    fn header_actions(&self, w: Width, cx: &mut Context<Self>) -> Vec<AnyElement> {
        // Trying out the sync chip here first (kit-first rule).
        let sync: Vec<AnyElement> =
            self.sync_chip("ov-sync", cx).into_iter().map(|c| div().flex().items_center().h(px(32.)).child(c).into_any_element()).collect();
        let customize = Button::new("ov-customize")
            .ghost()
            .icon(IconName::SlidersHorizontal)
            .tooltip(t("Arrange your Overview", "Organiza tu Resumen"))
            .on_click(cx.listener(|this, _, _, cx| this.set_customizing(true, cx)))
            .into_any_element();
        let mut actions = self.main_actions(w, cx);
        actions.splice(0..0, sync);
        actions.insert(0, customize);
        actions
    }

    fn main_actions(&self, w: Width, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if !self.can(Need::Record) {
            return Vec::new();
        }
        let pay = Button::new("ov-pay")
            .primary()
            .icon(IconName::Plus)
            .label(t("Record payment", "Registrar pago"))
            .tooltip("Ctrl+P")
            .on_click(cx.listener(|this, _, w, cx| this.open_payment_form(None, w, cx)))
            .into_any_element();
        let expense = cx.listener(|this, _, w, cx| this.open_entry_form(EntryKind::Expense, None, w, cx));
        let loan = cx.listener(|this, _, w, cx| this.open_loan_form(None, w, cx));
        let manage = self.can(Need::Manage);
        if w >= Width::Wide {
            let mut actions = vec![
                Button::new("ov-expense")
                    .icon(IconName::ReceiptText)
                    .label(t("New record", "Nuevo registro"))
                    .tooltip("Ctrl+E")
                    .on_click(expense)
                    .into_any_element(),
            ];
            if manage {
                actions.push(
                    Button::new("ov-new-loan").label(t("New loan", "Nuevo préstamo")).tooltip("Ctrl+L").on_click(loan).into_any_element(),
                );
            }
            actions.push(pay);
            return actions;
        }
        let more = Dropdown::new("ov-more").icon(IconName::Ellipsis).tooltip(t("More actions", "Más acciones")).align_end();
        let more = if manage {
            more.item(DropdownItem::new(t("New loan", "Nuevo préstamo"), loan).icon(IconName::HandCoins).hint("Ctrl+L"))
                .item(DropdownItem::new(t("New record", "Nuevo registro"), expense).icon(IconName::ReceiptText).hint("Ctrl+E"))
                .item(
                    DropdownItem::new(
                        t("Add capital", "Agregar capital"),
                        cx.listener(|this, _, w, cx| this.open_capital_form(CapitalKind::Contribution, w, cx)),
                    )
                    .icon(IconName::Landmark),
                )
        } else {
            more.item(DropdownItem::new(t("New record", "Nuevo registro"), expense).icon(IconName::ReceiptText).hint("Ctrl+E"))
        };
        vec![more.into_any_element(), pay]
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
                t("See the interest per period before you confirm.", "Mira el interés por período antes de confirmar."),
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
                let fees_sub = if s.fees_collected > 0 {
                    let fees = money.format(s.fees_collected);
                    tr!("plus {fees} in fees", "más {fees} en cargos")
                } else {
                    t("Received, all time", "Cobrado en total").to_string()
                };
                let st = Stat::new(t("Interest earned", "Interés cobrado"), money.format(s.interest_collected))
                    .icon(IconName::TrendingUp)
                    .chip(p.positive, p.positive_tint)
                    .caption(sel(fees_sub));
                counted(st, s.interest_collected)
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
        let animated = matches!(slot, ChartSlot::Overview);
        let chart = LineChart { labels, hover_labels, series, height, money: self.settings().money(), animated };
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
        let bar = h_flex().h(px(10.)).w_full().rounded(px(3.)).overflow_hidden().gap(px(2.)).bg(p.neutral_tint).children(
            parts
                .iter()
                .filter(|(_, v, _)| *v > 0 && total > 0)
                .map(|(_, v, c)| div().h_full().flex_basis(px(0.)).flex_grow(*v as f32 / total as f32).bg(*c)),
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
