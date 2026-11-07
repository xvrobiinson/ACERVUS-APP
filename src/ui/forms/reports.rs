//! The Reports window (the nav's Reports, admins and viewers), the same three
//! reports as the web app: a month's portfolio statement, collections in a
//! period by client or by collector, and the loans overdue on a date. Each
//! one exports as a PDF (a Letter page that opens in the browser's print
//! window, like receipts) or as an Excel workbook. Everything is worked out
//! on this PC from the open books.

use std::collections::HashMap;
use std::rc::Rc;

use chrono::NaiveDate;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::date_field::{DateChanged, DateField};
use super::{DATE_WIDTH, date_field, field, field_row, panel, picked_date};
use crate::app::VzApp;
use crate::i18n::t;
use crate::services::Snapshot;
use crate::services::reports::{self, Band, By, Ctx, Report};
use crate::tr;
use crate::ui::components::icon;
use crate::ui::kit::{Button, Dropdown, DropdownItem, Segmented};
use crate::ui::theme::{Typeset as _, palette, space, tabular, text};
use crate::utils::dates::{month_start, month_year};

/// How a report leaves the app.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Pdf,
    Excel,
}

pub struct ReportsForm {
    app: WeakEntity<VzApp>,
    snap: Rc<Snapshot>,
    today: NaiveDate,
    collectors: HashMap<i64, String>,
    months: Vec<NaiveDate>,
    month: NaiveDate,
    from: Entity<DateField>,
    to: Entity<DateField>,
    by: By,
    as_of: Entity<DateField>,
    band: Band,
    _subs: Vec<Subscription>,
}

impl ReportsForm {
    pub fn new(
        app: WeakEntity<VzApp>,
        snap: Rc<Snapshot>,
        today: NaiveDate,
        collectors: HashMap<i64, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let months = reports::months(&snap, today);
        // Early in a month, last month's statement is the one wanted.
        let month = if chrono::Datelike::day(&today) <= 5 && months.len() > 1 { months[1] } else { months[0] };
        let from = date_field(month_start(today), window, cx);
        let to = date_field(today, window, cx);
        let as_of = date_field(today, window, cx);
        let subs = [&from, &to, &as_of].map(|d| cx.subscribe(d, |_, _, _: &DateChanged, cx| cx.notify())).into_iter().collect();
        Self { app, snap, today, collectors, months, month, from, to, by: By::Client, as_of, band: Band::All, _subs: subs }
    }

    pub fn first_focus(&self, cx: &App) -> FocusHandle {
        self.from.read(cx).focus_handle(cx)
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx { snap: &self.snap, today: self.today, collectors: self.collectors.clone() }
    }

    fn range(&self, cx: &App) -> Option<(NaiveDate, NaiveDate)> {
        let (from, to) = (picked_date(&self.from, cx)?, picked_date(&self.to, cx)?);
        (from <= to).then_some((from, to.min(self.today)))
    }

    fn export(&self, report: Report, out: Output, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.app.update(cx, |app, cx| app.save_report(&report, out, window, cx));
    }

    /// Export PDF and Excel for one report, then a short line about it.
    fn actions(
        &self,
        id: &'static str,
        make: impl Fn(&Self, &App) -> Option<Report> + Clone + 'static,
        meta: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = palette(cx);
        let ok = make(self, cx).is_some();
        let pdf = make.clone();
        h_flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::S))
            .child(
                Button::new(SharedString::from(format!("{id}-pdf")))
                    .primary()
                    .small()
                    .icon(IconName::Printer)
                    .label(t("Export PDF", "Exportar PDF"))
                    .disabled(!ok)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if let Some(r) = pdf(this, cx) {
                            this.export(r, Output::Pdf, w, cx)
                        }
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("{id}-xlsx")))
                    .small()
                    .icon(IconName::FileSpreadsheet)
                    .label("Excel")
                    .disabled(!ok)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if let Some(r) = make(this, cx) {
                            this.export(r, Output::Excel, w, cx)
                        }
                    })),
            )
            .children(meta.map(|m| div().ml(px(space::XS)).typeset(text::CAPTION).font_features(tabular()).text_color(p.muted).child(m)))
    }
}

/// One report: its icon, name and what it holds over its controls.
fn card(cx: &App, glyph: IconName, title: &str, body: &str) -> Div {
    let p = palette(cx);
    panel(&p).gap(px(space::M)).child(
        h_flex()
            .items_start()
            .gap(px(space::M))
            .child(div().flex_none().size(px(34.)).rounded(px(10.)).bg(p.accent_tint).flex().items_center().justify_center().child(icon(
                glyph,
                p.accent_text,
                17.,
            )))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(title.to_string()))
                    .child(div().typeset(text::CAPTION).text_color(p.muted).child(body.to_string())),
            ),
    )
}

impl Render for ReportsForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let money = self.snap.settings.money();
        let date_box = |d: &Entity<DateField>| div().w(px(DATE_WIDTH)).child(d.clone());

        // The monthly statement: a month to pick.
        let app = cx.entity().downgrade();
        let month_pick = Dropdown::new("rp-month").label(month_year(self.month)).items(self.months.iter().map(|m| {
            let (app, m) = (app.clone(), *m);
            DropdownItem::new(month_year(m), move |_, _, cx| {
                let _ = app.update(cx, |f, cx| {
                    f.month = m;
                    cx.notify();
                });
            })
            .selected(m == self.month)
        }));
        let month = self.month;
        let statement = card(
            cx,
            IconName::ChartColumn,
            t("Monthly portfolio statement", "Estado mensual del portafolio"),
            t(
                "Portfolio value, capital in and out, interest earned and every loan's balance for one month.",
                "Valor del portafolio, capital que entra y sale, interés cobrado y el saldo de cada préstamo en un mes.",
            ),
        )
        .child(field(&p, t("Month", "Mes"), div().w(px(220.)).child(month_pick), None, None))
        .child(self.actions("rp-statement", move |f, _| Some(reports::monthly_statement(&f.ctx(), month)), None, cx));

        // Collections: a period and how to group it.
        let range = self.range(cx);
        let collected = range.map(|(from, to)| reports::collections(&self.ctx(), from, to, self.by));
        let collected_line = match &collected {
            Some(r) => {
                let (n, sum) = (r.fact(0), money.format(r.fact(1)));
                tr!("{n} payment(s) · {sum}", "{n} pago(s) · {sum}")
            }
            None => t("Choose a start before the end.", "Elige un inicio antes del final.").to_string(),
        };
        let by = self.by;
        let group = Segmented::pick(
            "rp-by",
            [(By::Client, t("Client", "Cliente")), (By::Collector, t("Collector", "Cobrador"))],
            by,
            cx,
            |f: &mut Self, x, _, cx| {
                f.by = x;
                cx.notify();
            },
        );
        let collections = card(
            cx,
            IconName::Receipt,
            t("Collections report", "Reporte de cobros"),
            t(
                "Payments received in a period, grouped by client or by collector.",
                "Pagos recibidos en un período, por cliente o por cobrador.",
            ),
        )
        .child(
            field_row()
                .child(field(&p, t("From", "Desde"), date_box(&self.from), None, None))
                .child(field(&p, t("To", "Hasta"), date_box(&self.to), None, None))
                .child(field(&p, t("Group by", "Agrupar por"), group, None, None)),
        )
        .child(self.actions(
            "rp-collections",
            move |f, cx| f.range(cx).map(|(from, to)| reports::collections(&f.ctx(), from, to, by)),
            Some(collected_line),
            cx,
        ));

        // Overdue: a date and how late.
        let as_of = picked_date(&self.as_of, cx).map(|d| d.min(self.today));
        let band = self.band;
        let late_line = as_of.map(|d| {
            let r = reports::overdue(&self.ctx(), d, band);
            let (n, sum) = (r.fact(0), money.format(r.fact(1)));
            tr!("{n} loan(s) · {sum}", "{n} préstamo(s) · {sum}")
        });
        let bands = Segmented::pick("rp-band", Band::ALL.map(|b| (b, b.label())), band, cx, |f: &mut Self, x, _, cx| {
            f.band = x;
            cx.notify();
        });
        let overdue = card(
            cx,
            IconName::CircleAlert,
            t("Overdue report", "Reporte de atrasos"),
            t(
                "Every loan past due on a date, with days late, the amount behind and what's owed in total.",
                "Cada préstamo vencido en una fecha, con los días de atraso, el monto atrasado y el total adeudado.",
            ),
        )
        .child(field_row().child(field(&p, t("As of", "Al"), date_box(&self.as_of), None, None)).child(field(
            &p,
            t("Days late", "Días de atraso"),
            bands,
            None,
            None,
        )))
        .child(self.actions(
            "rp-overdue",
            move |f, cx| picked_date(&f.as_of, cx).map(|d| reports::overdue(&f.ctx(), d.min(f.today), band)),
            late_line,
            cx,
        ));

        v_flex()
            .gap(px(space::M))
            .child(div().typeset(text::BODY).text_color(p.muted).child(t("Pick a period, then export it.", "Elige un período y expórtalo.")))
            .child(statement)
            .child(collections)
            .child(overdue)
            .child(div().typeset(text::CAPTION).text_color(p.faint).child(t(
                "Export PDF opens the report in your browser's print window: choose “Save as PDF”. Letter paper. Excel opens the workbook.",
                "Exportar PDF abre el reporte en la ventana de impresión del navegador: elige “Guardar como PDF”. Papel carta. Excel abre el libro.",
            )))
    }
}
