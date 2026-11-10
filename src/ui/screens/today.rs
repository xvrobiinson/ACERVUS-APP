//! Two Overview blocks of the clean look (2026-11-09, the web's
//! src/screens/overview.tsx): "Needs attention today", up to three numbered
//! lines (late loans, collector payments to confirm, installments due this
//! week), and "Loans by status", one bar and a short list.

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{LoanFilter, Route, VzApp};
use crate::domain::ledger::DisplayStatus;
use crate::domain::money::Cents;
use crate::i18n::t;
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::{empty_state, icon, sel};
use crate::ui::kit::{Button, Card};
use crate::ui::theme::{Palette, Typeset as _, space, tabular, text};

/// Where a numbered line goes.
#[derive(Clone, Copy)]
enum Go {
    LateLoans,
    Confirm,
    Loans,
}

impl VzApp {
    fn go_today(&mut self, go: Go, window: &mut Window, cx: &mut Context<Self>) {
        match go {
            Go::LateLoans => {
                self.loan_filter = LoanFilter::Attention;
                self.navigate(Route::Loans, window, cx);
            }
            Go::Confirm => self.navigate(Route::Payments, window, cx),
            Go::Loans => self.navigate(Route::Loans, window, cx),
        }
    }

    /// Needs attention today: up to three numbered lines, each opening
    /// where it is dealt with.
    pub(crate) fn today_card(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let money = self.settings().money();
        let today = self.today;
        let week_end = today + chrono::Days::new(7);
        let mut late: Vec<_> = Vec::new();
        let mut due_week: Vec<Cents> = Vec::new();
        for l in &self.snap.book.loans {
            let q = l.delinquency(today);
            match l.display_status(today) {
                DisplayStatus::Overdue => late.push((l.client_id, q.overdue_amount, q.days_overdue)),
                // Only loans that still take payments.
                DisplayStatus::Active | DisplayStatus::DueSoon if q.next_due.is_some_and(|d| d >= today && d <= week_end) => {
                    due_week.push(q.next_due_amount);
                }
                _ => {}
            }
        }
        let waiting = if self.can(Need::Manage) { self.waiting_to_confirm() } else { 0 };
        let mut items: Vec<(Go, String, String)> = Vec::new();
        if !late.is_empty() {
            let n = late.len();
            let behind = money.format(late.iter().map(|x| x.1).sum());
            let (cid, _, d) = late.iter().copied().max_by_key(|x| x.2).unwrap_or((0, 0, 0));
            let who = self.snap.client_name(cid).to_string();
            let days = if d == 1 { t("1 day", "1 día").to_string() } else { tr!("{d} days", "{d} días") };
            let strong = if n == 1 {
                t("1 loan is late", "1 préstamo está atrasado").to_string()
            } else {
                tr!("{n} loans are late", "{n} préstamos están atrasados")
            };
            let rest = tr!(", {behind} behind. The oldest is {who}, {days}.", ", {behind} sin pagar. El más atrasado es {who}, {days}.");
            items.push((Go::LateLoans, strong, rest));
        }
        if waiting > 0 {
            let n = waiting;
            let (strong, rest) = if n == 1 {
                (
                    t("1 collector payment", "1 cobro de un cobrador").to_string(),
                    t(" is waiting for you to confirm.", " espera tu confirmación.").to_string(),
                )
            } else {
                (
                    tr!("{n} collector payments", "{n} cobros de cobradores"),
                    t(" are waiting for you to confirm.", " esperan tu confirmación.").to_string(),
                )
            };
            items.push((Go::Confirm, strong, rest));
        }
        if !due_week.is_empty() {
            let n = due_week.len();
            let sum = money.format(due_week.iter().sum());
            let (strong, rest) = if n == 1 {
                (t("1 installment", "1 cuota").to_string(), tr!(" falls due this week, {sum}.", " vence esta semana, {sum}."))
            } else {
                (tr!("{n} installments", "{n} cuotas"), tr!(" fall due this week, {sum} in all.", " vencen esta semana, {sum} en total."))
            };
            items.push((Go::Loans, strong, rest));
        }

        let head = h_flex()
            .gap(px(10.))
            .px(px(space::XL))
            .pt(px(space::L + 4.))
            .pb(px(space::M))
            .child(div().flex_none().size(px(26.)).rounded_full().bg(p.text).flex().items_center().justify_center().child(icon(
                IconName::CircleAlert,
                p.surface,
                14.,
            )))
            .child(div().typeset(text::HEADING).text_color(p.text).child(t("Needs attention today", "Requiere atención hoy")));
        let body = if items.is_empty() {
            empty_state(
                p,
                IconName::CircleCheck,
                t("All clear", "Todo en orden"),
                t(
                    "Nothing needs you today: no late loans and nothing due this week.",
                    "Nada requiere tu atención hoy: ningún préstamo atrasado y nada vence esta semana.",
                ),
                None,
            )
            .into_any_element()
        } else {
            let rows = items.into_iter().enumerate().map(|(i, (go, strong, rest))| {
                h_flex()
                    .id(("today-line", i))
                    .items_start()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(p.hero().glass_edge)
                    .bg(p.surface)
                    .cursor_pointer()
                    .hover(|d| d.bg(p.hover))
                    .typeset(text::BODY)
                    .child(
                        div()
                            .flex_none()
                            .font_weight(FontWeight::BOLD)
                            .font_features(tabular())
                            .text_color(p.faint)
                            .child(format!("{:02}", i + 1)),
                    )
                    .child(div().flex_1().min_w_0().text_color(p.text).child(sel(format!("{strong}{rest}"))).when(false, |d| d))
                    .on_click(cx.listener(move |this, _, w, cx| this.go_today(go, w, cx)))
            });
            v_flex()
                .px(px(space::XL))
                .pb(px(space::XL))
                .gap(px(8.))
                .children(rows)
                .when(!late.is_empty(), |d| {
                    d.child(
                        h_flex().pt(px(4.)).child(
                            Button::new("today-late")
                                .primary()
                                .small()
                                .label(t("Open late loans", "Ver préstamos atrasados"))
                                .on_click(cx.listener(|this, _, w, cx| this.go_today(Go::LateLoans, w, cx))),
                        ),
                    )
                })
                .into_any_element()
        };
        Card::new()
            .flex_1()
            .relative()
            .child(head)
            .child(body)
            .child(crate::ui::kit::tour_mark("today", &self.tour_targets))
            .into_any_element()
    }

    /// Loans by status: a bar and the count in each state, then how many
    /// are paid off.
    pub(crate) fn status_card(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let today = self.today;
        let (mut ok, mut soon, mut late, mut later, mut gone, mut paid) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        for l in &self.snap.book.loans {
            match l.display_status(today) {
                DisplayStatus::Active | DisplayStatus::Restructured => ok += 1,
                DisplayStatus::DueSoon => soon += 1,
                DisplayStatus::Overdue if l.delinquency(today).days_overdue > 30 => later += 1,
                DisplayStatus::Overdue => late += 1,
                DisplayStatus::Defaulted => gone += 1,
                DisplayStatus::PaidOff => paid += 1,
            }
        }
        let groups = [
            (t("On time", "Al día"), ok, p.positive),
            (t("Due soon", "Vence pronto"), soon, p.info),
            (t("Late up to 30 days", "Atrasados hasta 30 días"), late, p.warning_fill),
            (t("Late over 30 days", "Atrasados más de 30 días"), later, p.negative),
            (t("Defaulted", "Incobrables"), gone, p.faint),
        ];
        let total: usize = groups.iter().map(|g| g.1).sum();
        let all = Button::new("status-all")
            .ghost()
            .small()
            .label(t("All loans", "Préstamos"))
            .on_click(cx.listener(|this, _, w, cx| this.navigate(Route::Loans, w, cx)));
        let body = if total == 0 {
            empty_state(
                p,
                IconName::HandCoins,
                t("No loans yet", "Aún no hay préstamos"),
                t("Your loans show here by how they are being paid.", "Tus préstamos aparecen aquí según cómo se están pagando."),
                None,
            )
            .into_any_element()
        } else {
            let bar = crate::ui::kit::motion::fill_in(
                h_flex().h(px(8.)).gap(px(2.)).rounded_full().overflow_hidden().children(
                    groups
                        .iter()
                        .filter(|g| g.1 > 0)
                        .map(|g| div().h_full().min_w(px(3.)).flex_grow(g.1 as f32).flex_basis(px(0.)).bg(g.2)),
                ),
                "status-fill",
            );
            let list = v_flex().gap(px(9.)).typeset(text::BODY).children(groups.iter().map(|(label, n, color)| {
                let pct = (*n as f32 / total as f32 * 100.).round() as i64;
                h_flex()
                    .gap(px(8.))
                    .child(div().flex_none().size(px(9.)).rounded(px(3.)).bg(*color))
                    .child(div().flex_1().min_w_0().text_color(p.text).child(*label))
                    .child(div().flex_none().font_features(tabular()).text_color(p.muted).child(format!("{n} · {pct}%")))
            }));
            v_flex()
                .px(px(space::XL))
                .pb(px(space::XL))
                .pt(px(4.))
                .gap(px(12.))
                .child(bar)
                .child(list)
                .when(paid > 0, |d| {
                    let n = paid;
                    d.child(div().typeset(text::CAPTION).text_color(p.faint).child(if n == 1 {
                        t("1 loan paid off", "1 préstamo saldado").to_string()
                    } else {
                        tr!("{n} loans paid off", "{n} préstamos saldados")
                    }))
                })
                .into_any_element()
        };
        Card::new().flex_1().title(t("Loans by status", "Préstamos por estado")).action(all).child(body).into_any_element()
    }
}
