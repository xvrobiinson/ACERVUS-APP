//! The periods the Overview's Interest earned can show: today, this week
//! (from Monday), this month, this year, or all time. Each one also has the
//! same stretch of the period before ("vs last month" compares Nov 1–5
//! with Oct 1–5), so the comparison is fair partway through a period. The
//! web app does the same (src/lib/periods.ts there).

use chrono::{Datelike, Duration, Months, NaiveDate};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Period {
    Day,
    Week,
    #[default]
    Month,
    Year,
    All,
}

/// A run of days, both ends included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

impl Span {
    pub fn contains(&self, d: NaiveDate) -> bool {
        self.from <= d && d <= self.to
    }
}

impl Period {
    pub const ALL: [Period; 5] = [Period::Day, Period::Week, Period::Month, Period::Year, Period::All];

    pub fn key(self) -> &'static str {
        match self {
            Period::Day => "day",
            Period::Week => "week",
            Period::Month => "month",
            Period::Year => "year",
            Period::All => "all",
        }
    }

    pub fn from_key(k: &str) -> Self {
        Self::ALL.into_iter().find(|p| p.key() == k).unwrap_or_default()
    }

    /// This period up to `today`, and the same stretch of the one before
    /// (none for all time).
    pub fn spans(self, today: NaiveDate) -> Option<(Span, Span)> {
        let back = |d: NaiveDate, m: u32| d.checked_sub_months(Months::new(m)).unwrap_or(d);
        let (from, prev_from, prev_to) = match self {
            Period::All => return None,
            Period::Day => (today, today - Duration::days(1), today - Duration::days(1)),
            Period::Week => {
                let from = today - Duration::days(today.weekday().num_days_from_monday() as i64);
                (from, from - Duration::days(7), today - Duration::days(7))
            }
            Period::Month | Period::Year => {
                let (months, from) = if self == Period::Month {
                    (1, today.with_day(1).unwrap_or(today))
                } else {
                    (12, NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today))
                };
                let prev_from = back(from, months);
                // The same number of days in, never past that period's end.
                let prev_to = (prev_from + (today - from)).min(from - Duration::days(1));
                (from, prev_from, prev_to)
            }
        };
        Some((Span { from, to: today }, Span { from: prev_from, to: prev_to }))
    }
}

/// The interest of the payments dated within `span` (`(paid_on, interest)`
/// pairs, reversed payments already left out).
pub fn interest_in(payments: impl IntoIterator<Item = (NaiveDate, i64)>, span: Span) -> i64 {
    payments.into_iter().filter(|(d, _)| span.contains(*d)).map(|(_, i)| i).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    fn span(a: NaiveDate, b: NaiveDate) -> Span {
        Span { from: a, to: b }
    }

    #[test]
    fn weeks_start_on_monday_and_compare_the_same_days() {
        // 2026-11-05 is a Thursday.
        assert_eq!(
            Period::Week.spans(d(2026, 11, 5)),
            Some((span(d(2026, 11, 2), d(2026, 11, 5)), span(d(2026, 10, 26), d(2026, 10, 29))))
        );
        assert_eq!(Period::Week.spans(d(2026, 11, 2)).unwrap().0.from, d(2026, 11, 2));
        assert_eq!(Period::Week.spans(d(2026, 11, 8)).unwrap().0.from, d(2026, 11, 2));
    }

    #[test]
    fn months_and_years_to_date_never_past_the_earlier_end() {
        assert_eq!(Period::Month.spans(d(2026, 11, 5)), Some((span(d(2026, 11, 1), d(2026, 11, 5)), span(d(2026, 10, 1), d(2026, 10, 5)))));
        assert_eq!(Period::Month.spans(d(2026, 3, 31)).unwrap().1, span(d(2026, 2, 1), d(2026, 2, 28)));
        assert_eq!(Period::Year.spans(d(2026, 11, 5)), Some((span(d(2026, 1, 1), d(2026, 11, 5)), span(d(2025, 1, 1), d(2025, 11, 5)))));
        assert_eq!(Period::Day.spans(d(2026, 11, 5)).unwrap().1, span(d(2026, 11, 4), d(2026, 11, 4)));
        assert_eq!(Period::All.spans(d(2026, 11, 5)), None);
    }

    #[test]
    fn adds_the_interest_in_the_span() {
        let pays = [(d(2026, 10, 31), 100), (d(2026, 11, 1), 200), (d(2026, 11, 5), 300)];
        assert_eq!(interest_in(pays, span(d(2026, 11, 1), d(2026, 11, 5))), 500);
        assert_eq!(interest_in(pays, span(d(2026, 11, 6), d(2026, 11, 30))), 0);
    }

    #[test]
    fn keys_round_trip() {
        for p in Period::ALL {
            assert_eq!(Period::from_key(p.key()), p);
        }
        assert_eq!(Period::from_key("nonsense"), Period::Month);
    }
}
