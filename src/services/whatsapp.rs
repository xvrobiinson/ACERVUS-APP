//! WhatsApp, the regular app (2026-11-07, the owner's choice): the app
//! opens WhatsApp with the client's number and the message already written,
//! and the person sending presses Send, from their own number. Nothing goes
//! through a server. Two kinds of message: a reminder (the day before an
//! installment is due, on the day, and once it is late) and a payment
//! receipt as text. The same as the web app's (src/lib/whatsapp.ts).

use chrono::{Days, NaiveDate};

use super::Snapshot;
use super::receipt_text::{WaMsg, fill_wa};
use crate::domain::ledger::DisplayStatus;
use crate::domain::money::Cents;
use crate::i18n::t;
use crate::tr;

/// Why a client gets a reminder today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Due {
    Tomorrow,
    Today,
    /// Late, for this many days.
    Late(i64),
}

/// One reminder to send today.
#[derive(Clone, Debug, PartialEq)]
pub struct Reminder {
    pub loan_id: i64,
    pub client_id: i64,
    pub client: String,
    /// Ready for a wa.me link; none when the client has no usable number.
    pub phone: Option<String>,
    pub due: Due,
    /// What to pay: the installment due, or everything behind when late.
    pub amount: Cents,
    /// The due date, or since when it is late.
    pub on: NaiveDate,
}

impl Reminder {
    /// What it is for this day, so a sent one stays sent until tomorrow
    /// (and a reminder of a later stage is a new one).
    pub fn key(&self, today: NaiveDate) -> String {
        let stage = match self.due {
            Due::Tomorrow => "tomorrow",
            Due::Today => "today",
            Due::Late(_) => "late",
        };
        format!("{today}:{}:{stage}", self.loan_id)
    }
}

/// The country calling code a number without one is given, from the
/// business's currency: pesos and dollars +1 (the Dominican Republic and
/// the US share it), the Caribbean guilder +599 (Curaçao, Sint Maarten).
/// Dollars are also Bonaire's, Saba's and Statia's (+599, 7 digits).
fn calling_code(currency: &str) -> Option<&'static str> {
    match currency {
        "DOP" | "USD" => Some("1"),
        "XCG" | "ANG" => Some("599"),
        _ => None,
    }
}

/// A phone number as wa.me wants it: digits only, with its country code.
/// "+1 809-555-1234" and "809 555 1234" (pesos) both give "18095551234".
/// None when there are too few digits to be a phone number.
pub fn phone_for_link(raw: &str, currency: &str) -> Option<String> {
    let trimmed = raw.trim();
    let international = trimmed.starts_with('+') || trimmed.starts_with("00");
    let mut digits: String = trimmed.chars().filter(char::is_ascii_digit).collect();
    if trimmed.starts_with("00") {
        digits = digits[2..].to_string();
    }
    if digits.len() < 7 {
        return None;
    }
    if international {
        return Some(digits);
    }
    match calling_code(currency) {
        Some("1") => match digits.len() {
            // A 10-digit North American number gets the 1 it lacks.
            10 => Some(format!("1{digits}")),
            // 7 digits in dollars: Bonaire, Saba or Statia.
            7 if currency == "USD" => Some(format!("599{digits}")),
            // Written whole, with its country code.
            n if n >= 11 => Some(digits),
            // A Dominican number without its area code (809, 829 or 849)
            // can't be completed.
            _ => None,
        },
        // Curaçao's local numbers have 7 digits.
        Some(code) if digits.len() <= 8 => Some(format!("{code}{digits}")),
        _ => Some(digits),
    }
}

/// The link that opens WhatsApp with the message written.
pub fn link(phone: &str, text: &str) -> String {
    let mut q = String::new();
    for b in text.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => q.push(*b as char),
            _ => q.push_str(&format!("%{b:02X}")),
        }
    }
    format!("https://wa.me/{phone}?text={q}")
}

/// The reminders to send today: every performing loan whose installment is
/// due tomorrow or today, or that is late; most urgent first.
pub fn reminders(snap: &Snapshot, today: NaiveDate) -> Vec<Reminder> {
    let tomorrow = today + Days::new(1);
    let currency = &snap.settings.currency_code;
    let mut out: Vec<Reminder> = snap
        .book
        .loans
        .iter()
        .filter_map(|l| {
            let status = l.display_status(today);
            if !matches!(status, DisplayStatus::Active | DisplayStatus::DueSoon | DisplayStatus::Overdue) {
                return None;
            }
            let d = l.delinquency(today);
            let (due, amount, on) = if d.overdue_amount > 0 {
                (Due::Late(d.days_overdue), d.overdue_amount, d.overdue_since.unwrap_or(today))
            } else {
                match d.next_due {
                    Some(n) if n == today && d.next_due_amount > 0 => (Due::Today, d.next_due_amount, n),
                    Some(n) if n == tomorrow && d.next_due_amount > 0 => (Due::Tomorrow, d.next_due_amount, n),
                    _ => return None,
                }
            };
            let client = snap.client(l.client_id)?;
            Some(Reminder {
                loan_id: l.id,
                client_id: client.id,
                client: client.full_name.clone(),
                phone: client.phone.as_deref().and_then(|p| phone_for_link(p, currency)),
                due,
                amount,
                on,
            })
        })
        .collect();
    let urgency = |r: &Reminder| match r.due {
        Due::Late(days) => -days,
        Due::Today => 1,
        Due::Tomorrow => 2,
    };
    out.sort_by_key(|r| (urgency(r), r.loan_id));
    out
}

fn first_name(full: &str) -> &str {
    full.split_whitespace().next().unwrap_or(full)
}

/// The reminder's words, signed with the business's name.
/// In the business's own wording (Settings › WhatsApp messages), else the
/// built-in one.
pub fn reminder_text(r: &Reminder, snap: &Snapshot, business: &str) -> String {
    let money = snap.settings.money();
    let which = match r.due {
        Due::Tomorrow => WaMsg::Tomorrow,
        Due::Today => WaMsg::Today,
        Due::Late(_) => WaMsg::Late,
    };
    let tags = [
        ("client", first_name(&r.client).to_string()),
        ("amount", money.format(r.amount)),
        ("date", snap.settings.date_style.format(r.on)),
        ("loan", r.loan_id.to_string()),
        ("business", business.to_string()),
    ];
    fill_wa(&snap.settings.receipt_text.wa(which), &tags)
}

/// A payment's receipt as text: its number, date, amount and how it was
/// paid, how it was split and what is still owed (each as the business's
/// receipt switches allow).
pub fn receipt_text(snap: &Snapshot, payment_id: i64) -> Option<String> {
    let d = super::receipt::data(snap, payment_id)?;
    let s = &snap.settings;
    let show = s.receipt_show;
    let money = s.money();
    let m = |c: Cents| money.format(c);
    let (no, date, biz) = (d.payment_id, s.date_style.format(d.paid_on), d.business.clone());
    let client = snap.loan(d.loan_id).map(|l| snap.client_name(l.client_id).to_string()).unwrap_or_default();
    let tags = [
        ("client", first_name(&client).to_string()),
        ("amount", m(d.amount)),
        ("date", date.clone()),
        ("loan", d.loan_id.to_string()),
        ("business", biz.clone()),
    ];
    let mut lines = Vec::new();
    let top = fill_wa(&s.receipt_text.wa(WaMsg::ReceiptTop), &tags);
    if !top.trim().is_empty() {
        lines.push(top.trim().to_string());
        lines.push(String::new());
    }
    lines.push(tr!("Receipt #{no} — {biz}", "Recibo #{no} — {biz}"));
    lines.push(tr!("Date: {date}", "Fecha: {date}"));
    let paid = m(d.amount);
    lines.push(match d.paid_with {
        Some(w) => {
            let how = w.label();
            tr!("Paid: {paid} ({how})", "Pagado: {paid} ({how})")
        }
        None => tr!("Paid: {paid}", "Pagado: {paid}"),
    });
    if show.split {
        let (cap, int) = (m(d.principal), m(d.interest));
        let mut split = tr!("Capital {cap} · Interest {int}", "Capital {cap} · Interés {int}");
        if d.fee > 0 && show.late_fees {
            let fee = m(d.fee);
            split.push_str(&tr!(" · Late fee {fee}", " · Mora {fee}"));
        }
        lines.push(split);
    }
    if show.loan_number {
        let n = d.loan_id;
        lines.push(tr!("Loan #{n}", "Préstamo #{n}"));
    }
    // The business's short receipt also tells what to pay next.
    if let Some(n) = d.next.filter(|_| s.receipt_format == super::settings::ReceiptFormat::Next) {
        let amount = m(n.amount);
        let what = if n.interest { t("Next interest payment", "Próximo pago de interés") } else { t("Next payment", "Próximo pago") };
        lines.push(match n.on {
            Some(on) => {
                let on = s.date_style.format(on);
                tr!("{what}: {amount} by {on}", "{what}: {amount} a más tardar el {on}")
            }
            None => tr!("Due now: {amount}", "Vence ahora: {amount}"),
        });
        if n.late > 0 && n.on.is_some() {
            let late = m(n.late);
            lines.push(tr!("(includes {late} already late)", "(incluye {late} ya atrasado)"));
        }
    }
    if show.balance {
        let left = m(d.balance_after());
        lines.push(tr!("Still owed: {left}", "Saldo pendiente: {left}"));
    }
    let end = fill_wa(&s.receipt_text.wa(WaMsg::ReceiptEnd), &tags);
    if !end.trim().is_empty() {
        lines.push(end.trim().to_string());
    }
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::repositories::clients::ClientInput;
    use crate::domain::loans::{Frequency, LoanTerms};
    use crate::domain::money::Rate;
    use crate::domain::portfolio::CapitalKind;
    use crate::services::Store;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn phone_numbers_get_their_country_code() {
        assert_eq!(phone_for_link("+1 809-555-1234", "DOP").as_deref(), Some("18095551234"));
        assert_eq!(phone_for_link("(809) 555 1234", "DOP").as_deref(), Some("18095551234"));
        assert_eq!(phone_for_link("1 809 555 1234", "DOP").as_deref(), Some("18095551234"));
        assert_eq!(phone_for_link("9 512 3456", "XCG").as_deref(), Some("59995123456"));
        assert_eq!(phone_for_link("00599 9 512 3456", "DOP").as_deref(), Some("59995123456"));
        assert_eq!(phone_for_link("555", "DOP"), None);
        assert_eq!(phone_for_link("717 1234", "USD").as_deref(), Some("5997171234"));
        assert_eq!(phone_for_link("555 1234", "DOP"), None);
        assert_eq!(phone_for_link("212 555 1234", "USD").as_deref(), Some("12125551234"));
        assert_eq!(phone_for_link("", "DOP"), None);
    }

    #[test]
    fn links_carry_the_message() {
        let url = link("18095551234", "Hola José, ¿5 & 6?\n— Préstamos");
        assert!(url.starts_with("https://wa.me/18095551234?text=Hola%20Jos%C3%A9%2C%20%C2%BF5%20%26%206%3F%0A"), "{url}");
    }

    /// A 1,000 loan at 10% a month from Jan 1, 2026 (interest-only, due on
    /// the 1st): owed 100 every month from Feb 1, nothing paid.
    fn books() -> (Snapshot, i64) {
        let today = d(2026, 1, 1);
        let mut s = Store::in_memory();
        let cid = s
            .create_client(&ClientInput { full_name: "Marta Quiroga".into(), phone: "809 555 1234".into(), ..Default::default() })
            .unwrap();
        s.add_capital(CapitalKind::Contribution, 2_000_000, today, "", today).unwrap();
        let terms = LoanTerms::interest_only(100_000, Rate(100_000), Frequency::Monthly, today);
        let lid = s.create_loan(cid, &terms, "", today).unwrap();
        (s.snapshot().unwrap(), lid)
    }

    #[test]
    fn reminders_come_the_day_before_on_the_day_and_when_late() {
        let (snap, lid) = books();
        assert!(reminders(&snap, d(2026, 1, 20)).is_empty());
        let r = reminders(&snap, d(2026, 1, 31));
        assert_eq!((r.len(), r[0].due, r[0].loan_id), (1, Due::Tomorrow, lid));
        assert_eq!(r[0].phone.as_deref(), Some("18095551234"));
        assert_eq!(reminders(&snap, d(2026, 2, 1))[0].due, Due::Today);
        let late = &reminders(&snap, d(2026, 2, 5))[0];
        assert_eq!(late.due, Due::Late(4));
        assert_eq!(late.on, d(2026, 2, 1));
        // A reminder of a later stage is a new one; the same one keeps its key.
        assert_ne!(r[0].key(d(2026, 1, 31)), late.key(d(2026, 1, 31)));
        let text = reminder_text(late, &snap, "Préstamos Valdez");
        assert!(text.contains("Marta") && text.ends_with("— Préstamos Valdez"), "{text}");
    }

    #[test]
    fn a_receipt_reads_as_text() {
        let today = d(2026, 2, 1);
        let mut s = Store::in_memory();
        let cid = s.create_client(&ClientInput { full_name: "Marta Quiroga".into(), ..Default::default() }).unwrap();
        s.add_capital(CapitalKind::Contribution, 2_000_000, d(2026, 1, 1), "", today).unwrap();
        let terms = LoanTerms::interest_only(100_000, Rate(100_000), Frequency::Monthly, d(2026, 1, 1));
        let lid = s.create_loan(cid, &terms, "", today).unwrap();
        let pid = s.record_payment(lid, 60_000, 0, today, "", today).unwrap();
        let snap = s.snapshot().unwrap();
        let text = receipt_text(&snap, pid).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with(&format!("Receipt #{pid}")), "{text}");
        assert!(text.contains("Loan #") && text.contains("Still owed"), "{text}");
        assert!(receipt_text(&snap, 9999).is_none());
    }
}
