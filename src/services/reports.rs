//! The three reports in the Reports panel, the same as the web app's
//! (src/lib/reports.ts), worked out from the open books: a month's
//! portfolio statement, the payments collected in a period (by client or
//! by collector), and the loans overdue on a date. Each comes back as plain
//! tables, then becomes a printable Letter page (the browser's print window
//! saves the PDF, like receipts) or an Excel workbook.

use std::collections::HashMap;

use chrono::{Days, NaiveDate};

use super::Snapshot;
use super::xlsx::{self, Cell, Column, Kind, Sheet};
use crate::database::repositories::payments::Payment;
use crate::domain::ledger::DisplayStatus;
use crate::domain::money::Cents;
use crate::domain::portfolio::{CapitalKind, EntryKind};
use crate::i18n::{self, t};
use crate::tr;
use crate::utils::dates::{add_months, month_start};

pub struct Table {
    pub title: String,
    pub columns: Vec<(String, Kind)>,
    pub rows: Vec<Vec<Cell>>,
    pub total: Option<Vec<Cell>>,
}

pub struct Fact {
    pub label: String,
    pub value: Cell,
}

pub struct Report {
    /// File names start with it.
    pub file: String,
    pub title: String,
    pub period: String,
    pub facts: Vec<Fact>,
    pub tables: Vec<Table>,
}

impl Report {
    /// A fact's value as a number (a count or cents), for the panel's lines.
    pub fn fact(&self, i: usize) -> i64 {
        match self.facts.get(i).map(|f| &f.value) {
            Some(Cell::Money(c)) => *c,
            Some(Cell::Int(n)) => *n,
            _ => 0,
        }
    }
}

/// Collections grouped by the client or by who brought the money in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    Client,
    Collector,
}

/// Overdue loans by how late they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    All,
    UpTo30,
    UpTo90,
    Over90,
}

impl Band {
    pub const ALL: [Band; 4] = [Band::All, Band::UpTo30, Band::UpTo90, Band::Over90];

    pub fn label(self) -> &'static str {
        match self {
            Band::All => t("All overdue", "Todos los atrasados"),
            Band::UpTo30 => t("1–30 days", "1–30 días"),
            Band::UpTo90 => t("31–90 days", "31–90 días"),
            Band::Over90 => t("Over 90 days", "Más de 90 días"),
        }
    }

    fn holds(self, days: i64) -> bool {
        match self {
            Band::All => true,
            Band::UpTo30 => days <= 30,
            Band::UpTo90 => days > 30 && days <= 90,
            Band::Over90 => days > 90,
        }
    }
}

/// What every report reads.
pub struct Ctx<'a> {
    pub snap: &'a Snapshot,
    pub today: NaiveDate,
    /// Who brought each payment in (by payment number), from the cloud: the
    /// collector of a confirmed collector payment, else who recorded it.
    pub collectors: HashMap<i64, String>,
}

impl Ctx<'_> {
    fn day(&self, d: NaiveDate) -> String {
        self.snap.settings.date_style.format(d)
    }
    fn money(&self, c: Cents) -> String {
        self.snap.settings.money().format(c)
    }
    fn client_of(&self, loan_id: i64) -> String {
        self.snap.loan(loan_id).and_then(|l| self.snap.client(l.client_id)).map_or("—".into(), |c| c.full_name.clone())
    }
    fn collector_of(&self, payment_id: i64) -> String {
        self.collectors.get(&payment_id).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| "—".into())
    }
    /// Payments received between two days, both included, oldest first.
    fn payments_in(&self, from: NaiveDate, to: NaiveDate) -> Vec<&Payment> {
        let mut out: Vec<&Payment> =
            self.snap.payments.iter().filter(|p| !p.is_reversed() && p.paid_on >= from && p.paid_on <= to).collect();
        out.sort_by_key(|p| (p.paid_on, p.id));
        out
    }
}

/// A month's first and last day (the last no later than today).
pub fn month_span(month: NaiveDate, today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let from = month_start(month);
    let end = add_months(from, 1).pred_opt().unwrap_or(from);
    (from, end.min(today))
}

/// Months with activity, newest first (at most ten years).
pub fn months(snap: &Snapshot, today: NaiveDate) -> Vec<NaiveDate> {
    let first = month_start(snap.book.first_activity().unwrap_or(today).min(today));
    let mut out = Vec::new();
    let mut m = month_start(today);
    while m >= first && out.len() < 120 {
        out.push(m);
        m = m.checked_sub_months(chrono::Months::new(1)).unwrap_or(first - Days::new(1));
    }
    out
}

fn sum<'a>(xs: impl IntoIterator<Item = &'a Payment>, f: impl Fn(&Payment) -> Cents) -> Cents {
    xs.into_iter().map(f).sum()
}

fn col(label: &str, kind: Kind) -> (String, Kind) {
    (label.to_string(), kind)
}

fn money(label: &str, c: Cents) -> Fact {
    Fact { label: label.to_string(), value: Cell::Money(c) }
}

fn count(label: &str, n: usize) -> Fact {
    Fact { label: label.to_string(), value: Cell::Int(n as i64) }
}

pub fn monthly_statement(c: &Ctx, month: NaiveDate) -> Report {
    let snap = c.snap;
    let (from, to) = month_span(month, c.today);
    let before = snap.book.summary(from.pred_opt().unwrap_or(from));
    let after = snap.book.summary(to);
    let pays = c.payments_in(from, to);
    let new_loans: Vec<_> = snap.loans.iter().filter(|l| l.terms.start_date >= from && l.terms.start_date <= to).collect();
    let capital = |k: CapitalKind| -> Cents {
        snap.capital.iter().filter(|x| x.kind == k && x.occurred_on >= from && x.occurred_on <= to).map(|x| x.amount).sum()
    };
    let entries = |k: EntryKind| -> Cents {
        snap.entries.iter().filter(|x| x.kind == k && x.occurred_on >= from && x.occurred_on <= to).map(|x| x.amount).sum()
    };
    let mut loans: Vec<_> = snap
        .book
        .loans
        .iter()
        .filter(|l| l.facts.start_date <= to)
        .map(|l| (l, l.position(to), l.display_status(to)))
        .filter(|(l, pos, _)| pos.principal_outstanding > 0 || l.facts.start_date >= from || pos.last_event.is_none_or(|d| d >= from))
        .collect();
    loans.sort_by_key(|(l, ..)| l.id);
    let name = |client_id: i64| snap.client(client_id).map_or("—".to_string(), |x| x.full_name.clone());
    let total = t("Total", "Total");
    Report {
        file: format!("statement-{}", from.format("%Y-%m")),
        title: t("Monthly portfolio statement", "Estado mensual del portafolio").into(),
        period: format!("{} – {}", c.day(from), c.day(to)),
        facts: vec![
            money(t("Portfolio value at the start", "Valor del portafolio al inicio"), before.portfolio_value),
            money(t("Portfolio value at the end", "Valor del portafolio al final"), after.portfolio_value),
            money(t("Change in the month", "Cambio en el mes"), after.portfolio_value - before.portfolio_value),
            money(t("Capital invested", "Capital invertido"), after.net_invested()),
            count(t("Payments received", "Pagos recibidos"), pays.len()),
            money(t("Collected", "Cobrado"), sum(pays.iter().copied(), |p| p.amount)),
            money(t("Interest and late fees collected", "Interés y mora cobrados"), sum(pays.iter().copied(), |p| p.interest + p.fee)),
            count(t("New loans", "Préstamos nuevos"), new_loans.len()),
            money(t("Lent in new loans", "Prestado en préstamos nuevos"), new_loans.iter().map(|l| l.terms.principal).sum()),
            money(t("Capital added", "Capital agregado"), capital(CapitalKind::Contribution)),
            money(t("Capital withdrawn", "Capital retirado"), capital(CapitalKind::Withdrawal)),
            money(t("Expenses", "Gastos"), entries(EntryKind::Expense)),
            money(t("Other income", "Otros ingresos"), entries(EntryKind::Income)),
            money(t("Overdue at the end", "Atrasado al final"), after.overdue_amount),
            count(t("Loans overdue at the end", "Préstamos atrasados al final"), after.overdue_loans),
        ],
        tables: vec![
            Table {
                title: t("Loans at the end of the month", "Préstamos al final del mes").into(),
                columns: vec![
                    col(t("Loan", "Préstamo"), Kind::Int),
                    col(t("Client", "Cliente"), Kind::Text),
                    col(t("Started", "Inicio"), Kind::Date),
                    col(t("Lent", "Prestado"), Kind::Money),
                    col(t("Capital owed", "Capital adeudado"), Kind::Money),
                    col(t("Owed in total", "Total adeudado"), Kind::Money),
                    col(t("Status", "Estado"), Kind::Text),
                ],
                rows: loans
                    .iter()
                    .map(|(l, pos, status)| {
                        vec![
                            Cell::Int(l.id),
                            name(l.client_id).into(),
                            Cell::Date(l.facts.start_date),
                            Cell::Money(l.facts.principal),
                            Cell::Money(pos.principal_outstanding),
                            Cell::Money(pos.payoff()),
                            status.label().into(),
                        ]
                    })
                    .collect(),
                total: Some(vec![
                    total.into(),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Money(loans.iter().map(|(l, ..)| l.facts.principal).sum()),
                    Cell::Money(loans.iter().map(|(_, pos, _)| pos.principal_outstanding).sum()),
                    Cell::Money(loans.iter().map(|(_, pos, _)| pos.payoff()).sum()),
                    Cell::Empty,
                ]),
            },
            Table {
                title: t("Payments in the month", "Pagos del mes").into(),
                columns: vec![
                    col(t("Date", "Fecha"), Kind::Date),
                    col(t("Client", "Cliente"), Kind::Text),
                    col(t("Loan", "Préstamo"), Kind::Int),
                    col(t("Capital", "Capital"), Kind::Money),
                    col(t("Interest and fees", "Interés y mora"), Kind::Money),
                    col(t("Amount", "Monto"), Kind::Money),
                ],
                rows: pays
                    .iter()
                    .map(|p| {
                        vec![
                            Cell::Date(p.paid_on),
                            c.client_of(p.loan_id).into(),
                            Cell::Int(p.loan_id),
                            Cell::Money(p.principal),
                            Cell::Money(p.interest + p.fee),
                            Cell::Money(p.amount),
                        ]
                    })
                    .collect(),
                total: Some(vec![
                    total.into(),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Money(sum(pays.iter().copied(), |p| p.principal)),
                    Cell::Money(sum(pays.iter().copied(), |p| p.interest + p.fee)),
                    Cell::Money(sum(pays.iter().copied(), |p| p.amount)),
                ]),
            },
        ],
    }
}

pub fn collections(c: &Ctx, from: NaiveDate, to: NaiveDate, by: By) -> Report {
    let pays = c.payments_in(from, to);
    let key = |p: &Payment| match by {
        By::Client => c.client_of(p.loan_id),
        By::Collector => c.collector_of(p.id),
    };
    let mut groups: Vec<(String, Vec<&Payment>)> = Vec::new();
    for p in &pays {
        let k = key(p);
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, ps)) => ps.push(p),
            None => groups.push((k, vec![p])),
        }
    }
    // Biggest first; ties keep the order they first appeared in.
    groups.sort_by_key(|(_, ps)| std::cmp::Reverse(sum(ps.iter().copied(), |p| p.amount)));
    let total = t("Total", "Total");
    let all = |f: fn(&Payment) -> Cents| sum(pays.iter().copied(), f);
    Report {
        file: format!("collections-{from}-{to}"),
        title: t("Collections report", "Reporte de cobros").into(),
        period: format!("{} – {}", c.day(from), c.day(to)),
        facts: vec![
            count(t("Payments", "Pagos"), pays.len()),
            money(t("Collected", "Cobrado"), all(|p| p.amount)),
            money(t("Capital", "Capital"), all(|p| p.principal)),
            money(t("Interest and late fees", "Interés y mora"), all(|p| p.interest + p.fee)),
        ],
        tables: vec![
            Table {
                title: match by {
                    By::Client => t("By client", "Por cliente"),
                    By::Collector => t("By collector", "Por cobrador"),
                }
                .into(),
                columns: vec![
                    col(
                        match by {
                            By::Client => t("Client", "Cliente"),
                            By::Collector => t("Collector", "Cobrador"),
                        },
                        Kind::Text,
                    ),
                    col(t("Payments", "Pagos"), Kind::Int),
                    col(t("Capital", "Capital"), Kind::Money),
                    col(t("Interest and fees", "Interés y mora"), Kind::Money),
                    col(t("Collected", "Cobrado"), Kind::Money),
                ],
                rows: groups
                    .iter()
                    .map(|(k, ps)| {
                        vec![
                            k.clone().into(),
                            Cell::Int(ps.len() as i64),
                            Cell::Money(sum(ps.iter().copied(), |p| p.principal)),
                            Cell::Money(sum(ps.iter().copied(), |p| p.interest + p.fee)),
                            Cell::Money(sum(ps.iter().copied(), |p| p.amount)),
                        ]
                    })
                    .collect(),
                total: Some(vec![
                    total.into(),
                    Cell::Int(pays.len() as i64),
                    Cell::Money(all(|p| p.principal)),
                    Cell::Money(all(|p| p.interest + p.fee)),
                    Cell::Money(all(|p| p.amount)),
                ]),
            },
            Table {
                title: t("Every payment", "Cada pago").into(),
                columns: vec![
                    col(t("Date", "Fecha"), Kind::Date),
                    col(t("Client", "Cliente"), Kind::Text),
                    col(t("Loan", "Préstamo"), Kind::Int),
                    col(t("Collector", "Cobrador"), Kind::Text),
                    col(t("Paid with", "Pagado con"), Kind::Text),
                    col(t("Amount", "Monto"), Kind::Money),
                ],
                rows: pays
                    .iter()
                    .map(|p| {
                        vec![
                            Cell::Date(p.paid_on),
                            c.client_of(p.loan_id).into(),
                            Cell::Int(p.loan_id),
                            c.collector_of(p.id).into(),
                            p.paid_with.map_or(Cell::Empty, |w| w.label().into()),
                            Cell::Money(p.amount),
                        ]
                    })
                    .collect(),
                total: Some(vec![total.into(), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Money(all(|p| p.amount))]),
            },
        ],
    }
}

pub fn overdue(c: &Ctx, as_of: NaiveDate, band: Band) -> Report {
    let snap = c.snap;
    let mut late: Vec<_> = snap
        .book
        .loans
        .iter()
        .filter(|l| l.facts.start_date <= as_of && l.display_status(as_of) == DisplayStatus::Overdue)
        .map(|l| (l, l.delinquency(as_of), l.position(as_of)))
        .filter(|(_, d, _)| band.holds(d.days_overdue))
        .collect();
    late.sort_by_key(|(l, d, _)| (std::cmp::Reverse(d.days_overdue), l.id));
    let client = |id: i64| snap.client(id);
    let behind: Cents = late.iter().map(|(_, d, _)| d.overdue_amount).sum();
    let owed: Cents = late.iter().map(|(_, _, pos)| pos.payoff()).sum();
    let day = c.day(as_of);
    Report {
        file: format!("overdue-{as_of}"),
        title: t("Overdue report", "Reporte de atrasos").into(),
        period: tr!("As of {day}", "Al {day}"),
        facts: vec![
            count(t("Loans overdue", "Préstamos atrasados"), late.len()),
            money(t("Amount behind", "Monto atrasado"), behind),
            money(t("Owed in total on them", "Total adeudado en ellos"), owed),
        ],
        tables: vec![Table {
            title: t("Loans overdue", "Préstamos atrasados").into(),
            columns: vec![
                col(t("Loan", "Préstamo"), Kind::Int),
                col(t("Client", "Cliente"), Kind::Text),
                col(t("Phone", "Teléfono"), Kind::Text),
                col(t("Late since", "Atrasado desde"), Kind::Date),
                col(t("Days late", "Días de atraso"), Kind::Int),
                col(t("Amount behind", "Monto atrasado"), Kind::Money),
                col(t("Owed in total", "Total adeudado"), Kind::Money),
            ],
            rows: late
                .iter()
                .map(|(l, d, pos)| {
                    let cl = client(l.client_id);
                    vec![
                        Cell::Int(l.id),
                        cl.map_or("—".to_string(), |x| x.full_name.clone()).into(),
                        cl.and_then(|x| x.phone.clone()).unwrap_or_default().into(),
                        d.overdue_since.map_or(Cell::Empty, Cell::Date),
                        Cell::Int(d.days_overdue),
                        Cell::Money(d.overdue_amount),
                        Cell::Money(pos.payoff()),
                    ]
                })
                .collect(),
            total: Some(vec![
                t("Total", "Total").into(),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                Cell::Money(behind),
                Cell::Money(owed),
            ]),
        }],
    }
}

// ----- output --------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn show(c: &Ctx, v: &Cell) -> String {
    match v {
        Cell::Empty => String::new(),
        Cell::Text(s) => s.clone(),
        Cell::Money(m) => c.money(*m),
        Cell::Int(n) => n.to_string(),
        Cell::Date(d) => c.day(*d),
    }
}

/// A printable Letter page; it opens the print window by itself, where
/// "Save as PDF" makes the PDF (like receipts).
pub fn html(c: &Ctx, r: &Report, business: &str) -> String {
    let made = c.day(c.today);
    let generated = tr!("Made on {made} with Acervus Finance", "Hecho el {made} con Acervus Finance");
    let right = |k: Kind| if matches!(k, Kind::Money | Kind::Int) { r#" class="n""# } else { "" };
    let table = |tb: &Table| -> String {
        let head = format!("<h2>{}</h2>", esc(&tb.title));
        if tb.rows.is_empty() {
            return format!(r#"{head}<p class="none">{}</p>"#, esc(t("Nothing in this period.", "Nada en este período.")));
        }
        let line = |row: &[Cell], class: &str| {
            let cells: String = tb
                .columns
                .iter()
                .enumerate()
                .map(|(i, (_, k))| format!("<td{}>{}</td>", right(*k), esc(&row.get(i).map(|v| show(c, v)).unwrap_or_default())))
                .collect();
            format!("<tr{class}>{cells}</tr>")
        };
        let ths: String = tb.columns.iter().map(|(l, k)| format!("<th{}>{}</th>", right(*k), esc(l))).collect();
        let body: String = tb.rows.iter().map(|r| line(r, "")).collect();
        let total = tb.total.as_ref().map(|r| line(r, r#" class="total""#)).unwrap_or_default();
        format!("{head}<table><thead><tr>{ths}</tr></thead><tbody>{body}{total}</tbody></table>")
    };
    let facts: String =
        r.facts.iter().map(|f| format!("<div><span>{}</span><b>{}</b></div>", esc(&f.label), esc(&show(c, &f.value)))).collect();
    let tables: String = r.tables.iter().map(table).collect();
    format!(
        r#"<!doctype html><html lang="{lang}"><head><meta charset="utf-8"><title>{title}</title><style>
@page {{ size: letter; margin: 0.6in; }}
* {{ box-sizing: border-box; }}
html {{ background: #e9ecee; }}
body {{ margin: 24px auto; width: 816px; min-height: 1056px; background: #fff; color: #1a1d21; font: 12px/1.45 "Segoe UI", Inter, system-ui, sans-serif; padding: 56px; font-variant-numeric: tabular-nums; }}
@media print {{ html {{ background: none; }} body {{ width: auto; min-height: 0; margin: 0; padding: 0; }} .actions {{ display: none; }} }}
header {{ display: flex; justify-content: space-between; align-items: flex-end; border-bottom: 2px solid #1a1d21; padding-bottom: 12px; margin-bottom: 18px; }}
header .biz {{ font-size: 12px; color: #5f656b; }}
h1 {{ font-size: 22px; font-weight: 600; margin: 2px 0 0; }}
header .period {{ font-size: 12.5px; text-align: right; }}
.facts {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px 18px; margin-bottom: 20px; }}
.facts div {{ border-bottom: 1px solid #e3e6e9; padding: 4px 0 6px; }}
.facts span {{ display: block; color: #5f656b; font-size: 11px; }}
.facts b {{ font-weight: 600; font-size: 13px; }}
h2 {{ font-size: 13.5px; margin: 22px 0 8px; }}
table {{ width: 100%; border-collapse: collapse; }}
th {{ text-align: left; font-weight: 600; font-size: 11px; color: #5f656b; border-bottom: 1px solid #c9ced2; padding: 5px 6px; }}
td {{ padding: 5px 6px; border-bottom: 1px solid #eef0f2; }}
tr.total td {{ font-weight: 600; border-top: 1px solid #1a1d21; border-bottom: 0; }}
.n {{ text-align: right; white-space: nowrap; }}
tr {{ page-break-inside: avoid; }}
.none {{ color: #5f656b; }}
footer {{ margin-top: 28px; color: #6b7177; font-size: 10.5px; }}
.actions {{ position: fixed; top: 16px; right: 16px; }}
.actions button {{ font: 600 13px "Segoe UI", system-ui, sans-serif; padding: 8px 16px; border: 0; border-radius: 8px; background: #25282b; color: #fff; cursor: pointer; }}
</style></head><body>
<header><div><div class="biz">{business}</div><h1>{title}</h1></div><div class="period">{period}</div></header>
<section class="facts">{facts}</section>
{tables}
<footer>{generated}</footer>
<div class="actions"><button onclick="window.print()">{print}</button></div>
<script>window.addEventListener('load', function () {{ setTimeout(function () {{ window.print(); }}, 250); }});</script>
</body></html>"#,
        lang = i18n::get().key(),
        title = esc(&r.title),
        business = esc(business),
        period = esc(&r.period),
        generated = esc(&generated),
        print = esc(t("Print / PDF", "Imprimir / PDF")),
    )
}

/// The same report as an Excel workbook: the figures first, then each table.
pub fn workbook(r: &Report, business: &str) -> Vec<u8> {
    let mut sheets = vec![Sheet {
        name: t("Summary", "Resumen").into(),
        title: format!("{} · {business}", r.title),
        subtitle: Some(r.period.clone()),
        columns: vec![
            Column { label: t("Figure", "Cifra").into(), kind: Kind::Text, width: Some(38.0) },
            Column { label: t("Amount", "Monto").into(), kind: Kind::Money, width: Some(16.0) },
            Column { label: t("Count", "Cantidad").into(), kind: Kind::Int, width: Some(10.0) },
        ],
        rows: r
            .facts
            .iter()
            .map(|f| match &f.value {
                Cell::Int(_) => vec![f.label.clone().into(), Cell::Empty, f.value.clone()],
                v => vec![f.label.clone().into(), v.clone(), Cell::Empty],
            })
            .collect(),
        total: None,
    }];
    for tb in &r.tables {
        sheets.push(Sheet {
            name: tb.title.clone(),
            title: tb.title.clone(),
            subtitle: Some(format!("{business} · {}", r.period)),
            columns: tb.columns.iter().map(|(l, k)| Column { label: l.clone(), kind: *k, width: None }).collect(),
            rows: tb.rows.clone(),
            total: tb.total.clone(),
        });
    }
    xlsx::workbook(&sheets)
}

/// A file name part from the business's name: letters and digits joined by
/// hyphens, at most 40 characters.
pub fn file_safe(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(40).collect();
    if out.is_empty() { "acervus".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::repositories::clients::ClientInput;
    use crate::domain::loans::{Frequency, LoanTerms};
    use crate::domain::money::Rate;
    use crate::services::Store;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// 20,000 of capital and a 1,000 loan at 10% a month from Jan 1, 2026,
    /// on which 600 was paid on Feb 1 (100 interest, 500 capital); nothing
    /// since, so it is overdue from Mar 1.
    fn books(today: NaiveDate) -> (Snapshot, i64, i64) {
        let mut s = Store::in_memory();
        let cid = s.create_client(&ClientInput { full_name: "Marta <Quiroga>".into(), ..Default::default() }).unwrap();
        s.add_capital(CapitalKind::Contribution, 2_000_000, d(2026, 1, 1), "", today).unwrap();
        let terms = LoanTerms::interest_only(100_000, Rate(100_000), Frequency::Monthly, d(2026, 1, 1));
        let lid = s.create_loan(cid, &terms, "", today).unwrap();
        let pid = s.record_payment(lid, 60_000, 0, d(2026, 2, 1), "", today).unwrap();
        (s.snapshot().unwrap(), lid, pid)
    }

    #[test]
    fn collections_by_client_and_by_collector() {
        let today = d(2026, 6, 1);
        let (snap, _, pid) = books(today);
        let ctx = Ctx { snap: &snap, today, collectors: HashMap::from([(pid, "Ana".to_string())]) };
        let r = collections(&ctx, d(2026, 2, 1), d(2026, 2, 28), By::Client);
        assert_eq!((r.fact(0), r.fact(1), r.fact(2), r.fact(3)), (1, 60_000, 50_000, 10_000));
        assert_eq!(r.tables[0].rows[0][0], Cell::Text("Marta <Quiroga>".into()));
        let r = collections(&ctx, d(2026, 2, 1), d(2026, 2, 28), By::Collector);
        assert_eq!(r.tables[0].rows[0][0], Cell::Text("Ana".into()));
        // Nobody known: a dash, never an empty group.
        let ctx = Ctx { snap: &snap, today, collectors: HashMap::new() };
        let r = collections(&ctx, d(2026, 2, 1), d(2026, 2, 28), By::Collector);
        assert_eq!(r.tables[0].rows[0][0], Cell::Text("—".into()));
        // A period with nothing in it.
        assert_eq!(collections(&ctx, d(2026, 3, 1), d(2026, 3, 31), By::Client).fact(0), 0);
    }

    #[test]
    fn overdue_by_how_late() {
        let today = d(2026, 6, 1);
        let (snap, lid, _) = books(today);
        let ctx = Ctx { snap: &snap, today, collectors: HashMap::new() };
        let all = overdue(&ctx, today, Band::All);
        assert_eq!(all.fact(0), 1);
        assert_eq!(all.tables[0].rows[0][0], Cell::Int(lid));
        let Cell::Int(days) = all.tables[0].rows[0][4] else { panic!("days late") };
        assert!(days > 90, "{days}");
        assert_eq!(overdue(&ctx, today, Band::Over90).fact(0), 1);
        assert_eq!(overdue(&ctx, today, Band::UpTo30).fact(0), 0);
        // Before anything was due, nothing is late.
        assert_eq!(overdue(&ctx, d(2026, 1, 15), Band::All).fact(0), 0);
    }

    #[test]
    fn monthly_statement_and_its_outputs() {
        let today = d(2026, 6, 1);
        let (snap, lid, _) = books(today);
        let ctx = Ctx { snap: &snap, today, collectors: HashMap::new() };
        let r = monthly_statement(&ctx, d(2026, 2, 1));
        assert_eq!(r.file, "statement-2026-02");
        assert_eq!((r.fact(4), r.fact(5), r.fact(6)), (1, 60_000, 10_000));
        assert_eq!(r.tables[0].rows[0][0], Cell::Int(lid));
        assert_eq!(r.tables[1].rows.len(), 1);
        let page = html(&ctx, &r, "Préstamos & Co");
        assert!(page.contains("Préstamos &amp; Co") && page.contains("Marta &lt;Quiroga&gt;"));
        assert!(page.contains("window.print()"));
        assert_eq!(&workbook(&r, "Préstamos & Co")[..2], b"PK");
    }

    #[test]
    fn months_and_spans() {
        let today = d(2026, 6, 10);
        let (snap, _, _) = books(today);
        let m = months(&snap, today);
        assert_eq!(m.first(), Some(&d(2026, 6, 1)));
        assert_eq!(m.last(), Some(&d(2026, 1, 1)));
        assert_eq!(month_span(d(2026, 6, 1), today), (d(2026, 6, 1), today));
        assert_eq!(month_span(d(2026, 2, 1), today), (d(2026, 2, 1), d(2026, 2, 28)));
        assert_eq!(file_safe("VZ Capital Group, S.R.L."), "VZ-Capital-Group-S-R-L");
        assert_eq!(file_safe("—"), "acervus");
    }
}
