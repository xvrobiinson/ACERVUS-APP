//! The calendar of payments (2026-11-10, roadmap item 52; the web's
//! src/lib/calendar.ts): what falls due on each day and what was received,
//! read from the schedules the app already works out. Installment loans:
//! each installment on its due date. Interest-only loans: each period's
//! interest, and the next one.

use std::collections::BTreeMap;

use chrono::{Datelike, Days, NaiveDate};

use super::Snapshot;
use crate::domain::installments::InstallmentStatus;
use crate::domain::ledger::{ContractStatus, PeriodState, period_states};
use crate::domain::money::Cents;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DueState {
    Paid,
    Partly,
    Late,
    Upcoming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DueEntry {
    pub loan_id: i64,
    pub client_id: i64,
    /// What fell (or falls) due that day, and what is still unpaid of it.
    pub amount: Cents,
    pub left: Cents,
    pub state: DueState,
    /// An interest-only loan's interest (not an installment).
    pub interest: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaidEntry {
    pub payment_id: i64,
    pub loan_id: i64,
    pub client_id: i64,
    pub amount: Cents,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CalendarDay {
    pub due: Vec<DueEntry>,
    pub paid: Vec<PaidEntry>,
}

impl CalendarDay {
    /// All that fell due, what is still unpaid, whether any is late, and
    /// what was received.
    pub fn totals(&self) -> (Cents, Cents, bool, Cents) {
        (
            self.due.iter().map(|e| e.amount).sum(),
            self.due.iter().map(|e| e.left).sum(),
            self.due.iter().any(|e| e.state == DueState::Late),
            self.paid.iter().map(|p| p.amount).sum(),
        )
    }
}

/// Each day from \`from\` to \`to\` (inclusive) with something on it.
pub fn days(snap: &Snapshot, from: NaiveDate, to: NaiveDate, today: NaiveDate) -> BTreeMap<NaiveDate, CalendarDay> {
    let mut out: BTreeMap<NaiveDate, CalendarDay> = BTreeMap::new();
    let inside = |d: NaiveDate| d >= from && d <= to;
    for l in &snap.book.loans {
        // A defaulted or restructured loan takes no more payments: nothing
        // of it falls due (what it received still shows).
        if matches!(l.facts.status, ContractStatus::Defaulted | ContractStatus::Restructured) {
            continue;
        }
        if let Some(state) = l.plan_state(today) {
            for x in &state.installments {
                if !inside(x.due) {
                    continue;
                }
                let st = match x.status(today) {
                    InstallmentStatus::Paid => DueState::Paid,
                    InstallmentStatus::Overdue => DueState::Late,
                    InstallmentStatus::PartlyPaid => DueState::Partly,
                    _ => DueState::Upcoming,
                };
                out.entry(x.due).or_default().due.push(DueEntry {
                    loan_id: l.id,
                    client_id: l.client_id,
                    amount: x.total_due(),
                    left: x.remaining().max(0),
                    state: st,
                    interest: false,
                });
            }
        } else {
            let pos = l.position(today);
            let mut seen = Vec::new();
            for (period, st, paid) in period_states(&pos, today) {
                seen.push(period.due);
                if !inside(period.due) {
                    continue;
                }
                out.entry(period.due).or_default().due.push(DueEntry {
                    loan_id: l.id,
                    client_id: l.client_id,
                    amount: period.interest,
                    left: (period.interest - paid).max(0),
                    state: match st {
                        PeriodState::Paid => DueState::Paid,
                        PeriodState::Overdue => DueState::Late,
                        PeriodState::PartlyPaid => DueState::Partly,
                        PeriodState::Open => DueState::Upcoming,
                    },
                    interest: true,
                });
            }
            // The next interest payment, when it is beyond the periods so far.
            let q = l.delinquency(today);
            if l.facts.status == ContractStatus::Active
                && let Some(next) = q.next_due.filter(|d| inside(*d) && !seen.contains(d))
            {
                let amount = q.next_due_amount - q.overdue_amount;
                if amount > 0 {
                    out.entry(next).or_default().due.push(DueEntry {
                        loan_id: l.id,
                        client_id: l.client_id,
                        amount,
                        left: amount,
                        state: DueState::Upcoming,
                        interest: true,
                    });
                }
            }
        }
    }
    for p in snap.payments.iter().filter(|p| !p.is_reversed() && inside(p.paid_on)) {
        let client_id = snap.loan(p.loan_id).map(|l| l.client_id).unwrap_or_default();
        out.entry(p.paid_on).or_default().paid.push(PaidEntry { payment_id: p.id, loan_id: p.loan_id, client_id, amount: p.amount });
    }
    for d in out.values_mut() {
        d.due.sort_by_key(|e| (e.state != DueState::Late, snap.client_name(e.client_id).to_lowercase()));
    }
    out
}

/// The days a month view shows: whole weeks from the Monday on or before
/// the 1st to the Sunday on or after the month's last day.
pub fn month_grid(month: NaiveDate) -> Vec<NaiveDate> {
    let first = month.with_day(1).unwrap_or(month);
    let start = first - Days::new(u64::from(first.weekday().num_days_from_monday()));
    let next = first.checked_add_months(chrono::Months::new(1)).unwrap_or(first);
    let last = next - Days::new(1);
    let end = last + Days::new(u64::from(6 - last.weekday().num_days_from_monday()));
    start.iter_days().take_while(|d| *d <= end).collect()
}

/// The seven days (Monday first) of the week that holds \`d\`.
pub fn week_of(d: NaiveDate) -> Vec<NaiveDate> {
    let start = d - Days::new(u64::from(d.weekday().num_days_from_monday()));
    start.iter_days().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::installments::Instruction;
    use crate::domain::loans::{Frequency, LoanTerms, Method};
    use crate::domain::money::Rate;
    use crate::domain::portfolio::CapitalKind;
    use crate::services::{ClientInput, Store};

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn whole_weeks_monday_first() {
        let g = month_grid(d(2026, 11, 10));
        assert_eq!(g.first(), Some(&d(2026, 10, 26)));
        assert_eq!(g.last(), Some(&d(2026, 12, 6)));
        assert_eq!(g.len() % 7, 0);
        assert_eq!(week_of(d(2026, 11, 10))[0], d(2026, 11, 9));
    }

    #[test]
    fn installments_fall_on_their_due_dates_and_payments_on_their_days() {
        let today = d(2026, 3, 10);
        let mut s = Store::in_memory();
        let cid = s.create_client(&ClientInput { full_name: "Marta Quiroga".into(), ..Default::default() }).unwrap();
        s.add_capital(CapitalKind::Contribution, 2_000_000, d(2026, 1, 1), "", today).unwrap();
        let terms = LoanTerms {
            principal: 1_000_000,
            rate: Rate(100_000),
            interest_period: Frequency::Monthly,
            frequency: Frequency::Monthly,
            start_date: d(2026, 1, 1),
            method: Method::Declining,
            installments: Some(5),
            late_fee_rate: Rate(0),
        };
        let lid = s.create_loan(cid, &terms, "", today).unwrap();
        s.record_installment_payment(lid, 300_000, d(2026, 2, 1), "", &Instruction::Installment(1), d(2026, 2, 1)).unwrap();
        let snap = s.snapshot().unwrap();
        let days = days(&snap, d(2026, 1, 1), d(2026, 6, 30), today);
        let feb = &days[&d(2026, 2, 1)];
        assert_eq!(feb.due[0].state, DueState::Paid);
        assert_eq!(feb.paid.len(), 1);
        assert_eq!(days[&d(2026, 3, 1)].due[0].state, DueState::Late);
        assert_eq!(days[&d(2026, 4, 1)].due[0].state, DueState::Upcoming);
        let (due, left, late, _) = days[&d(2026, 3, 1)].totals();
        assert!(late && left == due && due > 0);
    }
}
