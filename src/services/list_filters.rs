//! "Filter by" on Payments, Loans and Clients (owner 2026-11-11, the list at
//! https://claude.ai/artifact/4TUA75JRUpiwcUet8fvHx2): what each page can be
//! filtered by and whether a row passes. Kept on this PC (the settings'
//! `filters_*`, never sent to the cloud) until cleared. The web's twin is
//! src/lib/list-filters.ts, with the same keys.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::ledger::DisplayStatus;
use crate::domain::periods::Period;

/// A stretch of days: a named one, or your own from–to (`Custom`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum When {
    #[serde(rename = "today")]
    Today,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "month")]
    Month,
    #[serde(rename = "3m")]
    ThreeMonths,
    #[serde(rename = "year")]
    Year,
    #[default]
    #[serde(rename = "all")]
    All,
    #[serde(rename = "custom")]
    Custom,
}

impl When {
    pub const ALL: [When; 7] = [When::Today, When::Week, When::Month, When::ThreeMonths, When::Year, When::All, When::Custom];

    /// A card's period (the Overview's Interest earned, Collected this month).
    pub fn of(p: Period) -> When {
        match p {
            Period::Day => When::Today,
            Period::Week => When::Week,
            Period::Month => When::Month,
            Period::Year => When::Year,
            Period::All => When::All,
        }
    }

    /// The days it covers, both ends included; None: every day. An open end
    /// of your own dates stays open.
    pub fn days(self, today: NaiveDate, from: Option<NaiveDate>, to: Option<NaiveDate>) -> Option<(NaiveDate, NaiveDate)> {
        let span = |p: Period| p.spans(today).map(|(now, _)| (now.from, now.to));
        match self {
            When::All => None,
            When::Today => span(Period::Day),
            When::Week => span(Period::Week),
            When::Month => span(Period::Month),
            When::Year => span(Period::Year),
            When::ThreeMonths => Some((today.checked_sub_months(chrono::Months::new(3)).unwrap_or(today), today)),
            When::Custom if from.is_none() && to.is_none() => None,
            When::Custom => Some((from.unwrap_or(NaiveDate::MIN), to.unwrap_or(NaiveDate::MAX))),
        }
    }
}

fn within(d: NaiveDate, days: Option<(NaiveDate, NaiveDate)>) -> bool {
    days.is_none_or(|(a, b)| d >= a && d <= b)
}

// ----- Payments -----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaymentsBy {
    pub when: When,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    /// Payment methods ("cash", "transfer", …).
    pub methods: Vec<String>,
    /// Who brought each payment in (user ids).
    pub by: Vec<String>,
    /// What it paid: "fee", "interestOnly", "capital".
    pub parts: Vec<String>,
}

impl Default for PaymentsBy {
    /// The page's start: the last three months.
    fn default() -> Self {
        Self { when: When::ThreeMonths, from: None, to: None, methods: Vec::new(), by: Vec::new(), parts: Vec::new() }
    }
}

pub const PAID_PARTS: [&str; 3] = ["fee", "interestOnly", "capital"];

/// What a payment paid, for "What it paid".
pub fn parts_of(principal: i64, interest: i64, fee: i64) -> Vec<&'static str> {
    let mut out = Vec::new();
    if fee > 0 {
        out.push("fee");
    }
    if principal > 0 {
        out.push("capital");
    } else if interest > 0 {
        out.push("interestOnly");
    }
    out
}

/// One payment, as the filter reads it.
pub struct PaymentRow<'a> {
    pub paid_on: NaiveDate,
    /// None for payments recorded before methods existed (they show "—").
    pub method: Option<&'a str>,
    pub by: &'a str,
    pub principal: i64,
    pub interest: i64,
    pub fee: i64,
}

impl PaymentsBy {
    pub fn passes(&self, p: &PaymentRow, today: NaiveDate) -> bool {
        within(p.paid_on, self.when.days(today, self.from, self.to))
            && (self.methods.is_empty() || p.method.is_some_and(|m| self.methods.iter().any(|x| x == m)))
            && (self.by.is_empty() || self.by.iter().any(|x| x == p.by))
            && (self.parts.is_empty() || parts_of(p.principal, p.interest, p.fee).iter().any(|x| self.parts.iter().any(|y| y == x)))
    }
}

// ----- Loans -----

/// A loan's status for the filter: the status chart's, restructured apart.
pub const LOAN_STATUSES: [&str; 7] = ["ok", "soon", "late", "later", "paid", "gone", "restructured"];

pub fn loan_status_key(s: DisplayStatus, days_late: i64) -> &'static str {
    match s {
        DisplayStatus::Active => "ok",
        DisplayStatus::DueSoon => "soon",
        DisplayStatus::Overdue if days_late <= crate::ui::components::LATE_OVER_DAYS => "late",
        DisplayStatus::Overdue => "later",
        DisplayStatus::PaidOff => "paid",
        DisplayStatus::Defaulted => "gone",
        DisplayStatus::Restructured => "restructured",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoansBy {
    pub statuses: Vec<String>,
    /// Collector user ids, "none" for clients nobody collects from.
    pub collectors: Vec<String>,
    pub methods: Vec<String>,
    pub frequencies: Vec<String>,
    /// By the loan's start date.
    pub when: When,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

pub struct LoanRow<'a> {
    pub status: &'a str,
    pub collector: Option<&'a str>,
    pub method: &'a str,
    pub frequency: &'a str,
    pub start: NaiveDate,
}

impl LoansBy {
    pub fn passes(&self, l: &LoanRow, today: NaiveDate) -> bool {
        let any = |list: &[String], v: &str| list.is_empty() || list.iter().any(|x| x == v);
        any(&self.statuses, l.status)
            && any(&self.collectors, l.collector.unwrap_or("none"))
            && any(&self.methods, l.method)
            && any(&self.frequencies, l.frequency)
            && within(l.start, self.when.days(today, self.from, self.to))
    }
}

// ----- Clients -----

pub const CLIENT_STATUSES: [&str; 3] = ["late", "ontime", "none"];

/// A client's status from their loans' (the same as their status word).
pub fn client_status_key(loans: &[DisplayStatus]) -> &'static str {
    if loans.contains(&DisplayStatus::Overdue) {
        "late"
    } else if loans.iter().any(|s| matches!(s, DisplayStatus::Active | DisplayStatus::DueSoon | DisplayStatus::Restructured)) {
        "ontime"
    } else {
        "none"
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientsBy {
    pub statuses: Vec<String>,
    pub collectors: Vec<String>,
    pub archived: bool,
}

impl ClientsBy {
    pub fn passes(&self, status: &str, collector: Option<&str>, archived: bool) -> bool {
        archived == self.archived
            && (self.statuses.is_empty() || self.statuses.iter().any(|x| x == status))
            && (self.collectors.is_empty() || self.collectors.iter().any(|x| x == collector.unwrap_or("none")))
    }
}

// ----- Kept on this PC -----

/// The setting's text; empty when it is the page's start.
pub fn to_setting<T: Serialize + Default + PartialEq>(f: &T) -> String {
    if *f == T::default() { String::new() } else { serde_json::to_string(f).unwrap_or_default() }
}

/// The filters kept in a setting, the page's start when none (or broken).
pub fn from_setting<T: for<'de> Deserialize<'de> + Default>(s: &str) -> T {
    serde_json::from_str(s).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn stretches_cover_what_their_names_say() {
        let today = d(2026, 11, 11);
        assert_eq!(When::Today.days(today, None, None), Some((today, today)));
        assert_eq!(When::Month.days(today, None, None), Some((d(2026, 11, 1), today)));
        assert_eq!(When::ThreeMonths.days(today, None, None), Some((d(2026, 8, 11), today)));
        assert_eq!(When::Year.days(today, None, None), Some((d(2026, 1, 1), today)));
        assert_eq!(When::All.days(today, None, None), None);
        assert_eq!(When::Custom.days(today, Some(d(2026, 1, 5)), None), Some((d(2026, 1, 5), NaiveDate::MAX)));
        assert_eq!(When::Custom.days(today, None, None), None);
    }

    #[test]
    fn payments_pass_by_period_method_who_and_what_they_paid() {
        let today = d(2026, 11, 11);
        let p = PaymentRow { paid_on: d(2026, 11, 2), method: Some("transfer"), by: "ana", principal: 0, interest: 500, fee: 25 };
        let start = PaymentsBy::default();
        assert!(start.passes(&p, today));
        assert!(!PaymentsBy { when: When::Week, ..start.clone() }.passes(&p, today));
        assert!(!PaymentsBy { methods: vec!["cash".into()], ..start.clone() }.passes(&p, today));
        assert!(PaymentsBy { by: vec!["ana".into()], ..start.clone() }.passes(&p, today));
        assert!(PaymentsBy { parts: vec!["fee".into()], ..start.clone() }.passes(&p, today));
        assert!(!PaymentsBy { parts: vec!["capital".into()], ..start.clone() }.passes(&p, today));
        // No method recorded shows "—" and matches no method.
        let old = PaymentRow { method: None, ..p };
        assert!(!PaymentsBy { methods: vec!["cash".into()], ..start }.passes(&old, today));
    }

    #[test]
    fn loans_and_clients_pass_like_the_web() {
        let today = d(2026, 11, 11);
        let l = LoanRow {
            status: loan_status_key(DisplayStatus::Overdue, 12),
            collector: None,
            method: "flat",
            frequency: "weekly",
            start: d(2026, 3, 1),
        };
        assert_eq!(l.status, "late");
        assert!(LoansBy { statuses: vec!["late".into(), "later".into()], ..Default::default() }.passes(&l, today));
        assert!(LoansBy { collectors: vec!["none".into()], ..Default::default() }.passes(&l, today));
        assert!(!LoansBy { when: When::Month, ..Default::default() }.passes(&l, today));
        assert_eq!(client_status_key(&[DisplayStatus::Active, DisplayStatus::Overdue]), "late");
        assert_eq!(client_status_key(&[DisplayStatus::PaidOff]), "none");
        let c = ClientsBy::default();
        assert!(c.passes("late", Some("ana"), false));
        assert!(!c.passes("late", Some("ana"), true));
        assert!(!ClientsBy { statuses: vec!["ontime".into()], ..c }.passes("late", None, false));
    }

    #[test]
    fn kept_filters_round_trip_and_the_start_is_kept_as_nothing() {
        assert_eq!(to_setting(&PaymentsBy::default()), "");
        let f = LoansBy { methods: vec!["flat".into()], ..Default::default() };
        assert_eq!(from_setting::<LoansBy>(&to_setting(&f)), f);
        assert_eq!(from_setting::<ClientsBy>("broken"), ClientsBy::default());
        assert_eq!(from_setting::<PaymentsBy>(""), PaymentsBy::default());
    }
}
