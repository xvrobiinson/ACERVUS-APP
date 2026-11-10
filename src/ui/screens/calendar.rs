//! Calendar (2026-11-10, roadmap item 52; the web's src/screens/calendar.tsx):
//! what falls due when, by month or by week, with each day's total; a day
//! opens its loans, to record a payment or send a reminder from there.
//! Collectors see their own clients only. Reads `services::calendar`.

use chrono::{Datelike, Days, Months, NaiveDate};
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::VzApp;
use crate::i18n::t;
use crate::services::calendar::{self, DueState};
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::{Tone, empty_state, page_header, pill};
use crate::ui::kit::{Button, Card, Segmented};
use crate::ui::theme::{Typeset as _, Width, palette, space, tabular, text};

/// Month or week.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum CalendarView {
    #[default]
    Month,
    Week,
}

impl VzApp {
    pub(crate) fn render_calendar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let w = Width::of(self.content_width(window));
        let money = self.settings().money();
        let ds = self.settings().date_style;
        let today = self.today;
        let anchor = self.calendar_anchor.unwrap_or(today);
        let picked = self.calendar_picked.unwrap_or(today);
        let view = self.calendar_view;
        let grid = match view {
            CalendarView::Month => calendar::month_grid(anchor),
            CalendarView::Week => calendar::week_of(anchor),
        };
        let (first, last) = (grid[0], grid[grid.len() - 1]);
        let days = calendar::days(&self.snap, first, last, today);
        let month = (anchor.year(), anchor.month());
        let title = match view {
            CalendarView::Month => {
                let name = month_name(anchor.month());
                format!("{name} {}", anchor.year())
            }
            CalendarView::Week => format!("{} – {}", ds.format(first), ds.format(last)),
        };
        // The period's totals, for the line under the title.
        let in_period = |d: &NaiveDate| view == CalendarView::Week || (d.year(), d.month()) == month;
        let (left, received) = days.iter().filter(|(d, _)| in_period(d)).fold((0, 0), |(l, r), (_, x)| {
            let (_, left, _, paid) = x.totals();
            (l + left, r + paid)
        });

        let step = move |dir: i32| {
            move |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                let a = this.calendar_anchor.unwrap_or(this.today);
                this.calendar_anchor = Some(match this.calendar_view {
                    CalendarView::Week if dir > 0 => a + Days::new(7),
                    CalendarView::Week => a - Days::new(7),
                    CalendarView::Month if dir > 0 => a.checked_add_months(Months::new(1)).unwrap_or(a),
                    CalendarView::Month => a.checked_sub_months(Months::new(1)).unwrap_or(a),
                });
                cx.notify();
            }
        };
        let head = h_flex()
            .gap(px(6.))
            .px(px(space::L))
            .pt(px(space::L))
            .pb(px(space::S))
            .child(Button::new("cal-prev").ghost().small().icon(IconName::ChevronLeft).on_click(cx.listener(step(-1))))
            .child(div().min_w(px(170.)).text_center().typeset(text::HEADING).text_color(p.text).child(title))
            .child(Button::new("cal-next").ghost().small().icon(IconName::ChevronRight).on_click(cx.listener(step(1))))
            .child(div().flex_1())
            .child(Button::new("cal-today").small().label(t("Today", "Hoy")).on_click(cx.listener(|this, _, _, cx| {
                this.calendar_anchor = None;
                this.calendar_picked = None;
                cx.notify();
            })));
        let weekdays = h_flex().px(px(space::M)).gap(px(4.)).children(
            [t("Mon", "Lun"), t("Tue", "Mar"), t("Wed", "Mié"), t("Thu", "Jue"), t("Fri", "Vie"), t("Sat", "Sáb"), t("Sun", "Dom")]
                .map(|w| div().flex_1().flex_basis(px(0.)).py(px(4.)).text_center().typeset(text::CAPTION).text_color(p.faint).child(w)),
        );
        let tall = if view == CalendarView::Week { 150. } else { 78. };
        let cell = |d: NaiveDate, cx: &mut Context<Self>| {
            let x = days.get(&d);
            let (due, left, late, paid) = x.map(|x| x.totals()).unwrap_or_default();
            let out = view == CalendarView::Month && (d.year(), d.month()) != month;
            let names: Vec<String> = match (view, x) {
                (CalendarView::Week, Some(x)) => x
                    .due
                    .iter()
                    .take(4)
                    .map(|e| self.snap.client_name(e.client_id).split_whitespace().next().unwrap_or_default().to_string())
                    .collect(),
                _ => Vec::new(),
            };
            v_flex()
                .id(SharedString::from(format!("cal-{d}")))
                .relative()
                .flex_1()
                .flex_basis(px(0.))
                .min_w_0()
                .h(px(tall))
                .p(px(7.))
                .gap(px(2.))
                .rounded(px(12.))
                .border_1()
                .border_color(if d == picked { p.text } else { gpui_kit::transparent_black() })
                .bg(if d == picked { p.surface } else { p.hover })
                .when(out, |c| c.opacity(0.45))
                .cursor_pointer()
                .child(
                    h_flex().child(
                        div()
                            .px(px(5.))
                            .h(px(22.))
                            .min_w(px(22.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .typeset(text::CAPTION)
                            .font_weight(FontWeight::SEMIBOLD)
                            .font_features(tabular())
                            .when(d == today, |c| c.bg(p.text).text_color(p.surface))
                            .child(d.day().to_string()),
                    ),
                )
                .when(due > 0, |c| {
                    c.child(
                        div()
                            .typeset(text::CAPTION)
                            .font_weight(if left > 0 { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                            .font_features(tabular())
                            .text_color(if left > 0 { p.text } else { p.faint })
                            .when(left == 0, |c| c.line_through())
                            .child(money.format_compact(if left > 0 { left } else { due })),
                    )
                })
                .when(paid > 0, |c| {
                    c.child(
                        div()
                            .typeset(text::MICRO)
                            .font_features(tabular())
                            .text_color(p.positive)
                            .child(format!("+{}", money.format_compact(paid))),
                    )
                })
                .children(names.into_iter().map(|n| div().truncate().typeset(text::MICRO).text_color(p.muted).child(n)))
                .when(late, |c| c.child(div().absolute().top(px(9.)).right(px(9.)).size(px(7.)).rounded_full().bg(p.negative)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.calendar_picked = Some(d);
                    cx.notify();
                }))
        };
        let rows: Vec<AnyElement> = grid
            .chunks(7)
            .map(|week| h_flex().px(px(space::M)).gap(px(4.)).children(week.iter().map(|d| cell(*d, cx))).into_any_element())
            .collect();
        let calendar =
            Card::new().flex_1().min_w_0().child(head).child(weekdays).child(v_flex().gap(px(4.)).pb(px(space::M)).children(rows)).child(
                h_flex()
                    .gap(px(space::L))
                    .px(px(space::L))
                    .pb(px(space::L))
                    .typeset(text::CAPTION)
                    .text_color(p.muted)
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .child(div().size(px(7.)).rounded_full().bg(p.negative))
                            .child(t("Something late", "Algo atrasado")),
                    )
                    .child(h_flex().gap(px(5.)).child(div().size(px(7.)).rounded_full().bg(p.positive)).child(t("Received", "Recibido"))),
            );

        // The picked day: its loans, to record or remind, and what came in.
        let day = days.get(&picked);
        let record = self.can(Need::Record);
        let side_body: AnyElement = match day {
            None => empty_state(
                &p,
                IconName::CalendarDays,
                t("Nothing that day", "Nada ese día"),
                t("Nothing falls due and nothing was received this day.", "Nada vence y no se recibió nada este día."),
                None,
            )
            .into_any_element(),
            Some(x) => {
                // Today's WhatsApp reminders, once for every row.
                let reminders = self.wa_reminders();
                let dues = x.due.iter().enumerate().map(|(i, e)| {
                    let (lid, cid) = (e.loan_id, e.client_id);
                    let (label, tone) = match e.state {
                        DueState::Paid => (t("Paid", "Pagada"), Tone::Positive),
                        DueState::Partly => (t("Partly paid", "Pago parcial"), Tone::Warning),
                        DueState::Late => (t("Late", "Atrasada"), Tone::Negative),
                        DueState::Upcoming => (t("Due", "Vence"), Tone::Neutral),
                    };
                    let what = if e.interest {
                        tr!("Loan #{lid} · interest", "Préstamo #{lid} · interés")
                    } else {
                        tr!("Loan #{lid} · installment", "Préstamo #{lid} · cuota")
                    };
                    h_flex()
                        .gap(px(space::M))
                        .px(px(space::L))
                        .py(px(space::S))
                        .border_t_1()
                        .border_color(p.rule)
                        .child(
                            v_flex()
                                .id(("cal-due", i))
                                .flex_1()
                                .min_w_0()
                                .cursor_pointer()
                                .child(
                                    div()
                                        .truncate()
                                        .typeset(text::BODY_STRONG)
                                        .text_color(p.text)
                                        .child(self.snap.client_name(cid).to_string()),
                                )
                                .child(
                                    h_flex()
                                        .gap(px(6.))
                                        .typeset(text::CAPTION)
                                        .text_color(p.muted)
                                        .child(what)
                                        .child(pill(&p, label, tone)),
                                )
                                .on_click(cx.listener(move |this, _, w, cx| this.navigate(crate::app::Route::Loan(lid), w, cx))),
                        )
                        .child(
                            div()
                                .flex_none()
                                .typeset(text::BODY_STRONG)
                                .font_features(tabular())
                                .text_color(if e.state == DueState::Late { p.negative } else { p.text })
                                .child(money.format(if e.left > 0 { e.left } else { e.amount })),
                        )
                        .when(e.left > 0 && record, |d| {
                            d.child(
                                Button::new(("cal-pay", i))
                                    .ghost()
                                    .small()
                                    .icon(IconName::ReceiptText)
                                    .on_click(cx.listener(move |this, _, w, cx| this.open_payment_form(Some(lid), w, cx))),
                            )
                        })
                        .when(e.left > 0, |d| d.children(self.whatsapp_button_among(&reminders, cid, Some(lid), cx)))
                        .into_any_element()
                });
                let paid = x.paid.iter().map(|pe| {
                    let lid = pe.loan_id;
                    h_flex()
                        .gap(px(space::M))
                        .px(px(space::L))
                        .py(px(space::S))
                        .border_t_1()
                        .border_color(p.rule)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .typeset(text::BODY_STRONG)
                                        .text_color(p.text)
                                        .child(self.snap.client_name(pe.client_id).to_string()),
                                )
                                .child(div().typeset(text::CAPTION).text_color(p.muted).child(tr!("Loan #{lid}", "Préstamo #{lid}"))),
                        )
                        .child(
                            div()
                                .typeset(text::BODY_STRONG)
                                .font_features(tabular())
                                .text_color(p.positive)
                                .child(format!("+{}", money.format(pe.amount))),
                        )
                        .into_any_element()
                });
                v_flex()
                    .pb(px(space::S))
                    .children(dues)
                    .when(!x.paid.is_empty(), |d| {
                        d.child(
                            h_flex()
                                .gap(px(6.))
                                .px(px(space::L))
                                .pt(px(space::M))
                                .pb(px(4.))
                                .typeset(text::CAPTION)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.positive)
                                .child(t("Received this day", "Recibido este día")),
                        )
                    })
                    .children(paid)
                    .into_any_element()
            }
        };
        let (_, day_left, _, _) = day.map(|x| x.totals()).unwrap_or_default();
        let side = Card::new()
            .title(ds.format(picked))
            .when(day_left > 0, |c| {
                let l = money.format(day_left);
                c.action(div().typeset(text::CAPTION).text_color(p.muted).child(tr!("{l} to collect", "{l} por cobrar")))
            })
            .child(side_body);

        let view_pick = Segmented::pick(
            "cal-view",
            [(CalendarView::Month, SharedString::from(t("Month", "Mes"))), (CalendarView::Week, SharedString::from(t("Week", "Semana")))],
            view,
            cx,
            |this: &mut Self, v, _, cx| {
                this.calendar_view = v;
                cx.notify();
            },
        );
        let summary = {
            let (l, r) = (money.format(left), money.format(received));
            tr!("{l} still to collect · {r} received", "{l} por cobrar · {r} recibido")
        };
        v_flex()
            .gap(px(w.gap()))
            .child(page_header(
                &p,
                t("Calendar", "Calendario"),
                Some(div().child(summary).into_any_element()),
                vec![view_pick.into_any_element()],
            ))
            .child(if w >= Width::Wide {
                h_flex().items_start().gap(px(w.gap())).child(calendar).child(div().w(px(400.)).flex_none().child(side)).into_any_element()
            } else {
                v_flex().gap(px(w.gap())).child(calendar).child(side).into_any_element()
            })
            .into_any_element()
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => t("January", "Enero"),
        2 => t("February", "Febrero"),
        3 => t("March", "Marzo"),
        4 => t("April", "Abril"),
        5 => t("May", "Mayo"),
        6 => t("June", "Junio"),
        7 => t("July", "Julio"),
        8 => t("August", "Agosto"),
        9 => t("September", "Septiembre"),
        10 => t("October", "Octubre"),
        11 => t("November", "Noviembre"),
        _ => t("December", "Diciembre"),
    }
}
