//! The four figures in the Overview's hero, as each person chose them, the
//! same as the web app's (src/lib/hero-layout.ts) and saved to the person's
//! account (`prefs:mine` / `prefs:save`), so both apps show the same: per
//! place a big figure and two small rows under it, any of which can be
//! "none" (left empty). A place with nothing in it is not shown, and a
//! place left with one row shows that row larger. Which figures a person may
//! pick follows their role: the business's totals are for admins only.

use serde_json::{Value, json};

use crate::i18n::t;
use crate::sync::wire::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Figure {
    Portfolio,
    LentOut,
    InterestEarned,
    AvailableCapital,
    AvailableInterest,
    Receivable,
    Overdue,
    CollectedMonth,
    ExpectedInterest,
    DueNext7,
    Gain6m,
    ActiveLoans,
    OverdueLoans,
    ToConfirm,
}

impl Figure {
    pub const ALL: [Figure; 14] = [
        Figure::Portfolio,
        Figure::LentOut,
        Figure::InterestEarned,
        Figure::AvailableCapital,
        Figure::AvailableInterest,
        Figure::Receivable,
        Figure::Overdue,
        Figure::CollectedMonth,
        Figure::ExpectedInterest,
        Figure::DueNext7,
        Figure::Gain6m,
        Figure::ActiveLoans,
        Figure::OverdueLoans,
        Figure::ToConfirm,
    ];

    /// The web's name for it, as saved.
    pub fn key(self) -> &'static str {
        match self {
            Figure::Portfolio => "portfolio",
            Figure::LentOut => "lentOut",
            Figure::InterestEarned => "interestEarned",
            Figure::AvailableCapital => "availableCapital",
            Figure::AvailableInterest => "availableInterest",
            Figure::Receivable => "receivable",
            Figure::Overdue => "overdue",
            Figure::CollectedMonth => "collectedMonth",
            Figure::ExpectedInterest => "expectedInterest",
            Figure::DueNext7 => "dueNext7",
            Figure::Gain6m => "gain6m",
            Figure::ActiveLoans => "activeLoans",
            Figure::OverdueLoans => "overdueLoans",
            Figure::ToConfirm => "toConfirm",
        }
    }

    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.key() == k)
    }

    pub fn label(self) -> &'static str {
        match self {
            Figure::Portfolio => t("Total portfolio value", "Valor total de la cartera"),
            Figure::LentOut => t("Capital lent out", "Capital prestado"),
            Figure::InterestEarned => t("Interest earned", "Interés cobrado"),
            Figure::AvailableCapital => t("Available capital", "Capital disponible"),
            Figure::AvailableInterest => t("Available interest", "Interés disponible"),
            Figure::Receivable => t("Receivable", "Por cobrar"),
            Figure::Overdue => t("Overdue amount", "Monto atrasado"),
            Figure::CollectedMonth => t("Collected this month", "Cobrado este mes"),
            Figure::ExpectedInterest => t("Expected interest / month", "Interés esperado / mes"),
            Figure::DueNext7 => t("Due in next 7 days", "Vence en 7 días"),
            Figure::Gain6m => t("Gain (6M)", "Ganancia (6M)"),
            Figure::ActiveLoans => t("Active loans", "Préstamos activos"),
            Figure::OverdueLoans => t("Loans overdue", "Préstamos atrasados"),
            Figure::ToConfirm => t("To confirm", "Por confirmar"),
        }
    }

    /// Reads as a sentence, not an amount: small rows only.
    pub fn row_only(self) -> bool {
        self == Figure::Gain6m
    }
}

/// One spot: a figure, or left empty.
pub type Pick = Option<Figure>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub big: Pick,
    pub rows: [Pick; 2],
}

/// Which spot of a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Big,
    Row(usize),
}

pub type Layout = [Slot; 4];

/// What a role may see. Collectors and viewers never see the business's
/// totals (portfolio, capital, interest), the same rule as the rest of the
/// app. No role means this PC's own books: everything but To confirm.
pub fn allowed(role: Option<Role>) -> Vec<Figure> {
    let team = [Figure::Overdue, Figure::OverdueLoans, Figure::Receivable, Figure::DueNext7, Figure::ActiveLoans];
    match role {
        None | Some(Role::Admin) => Figure::ALL.into_iter().filter(|f| *f != Figure::ToConfirm).collect(),
        Some(Role::Collector) => team.into_iter().chain([Figure::ToConfirm]).collect(),
        Some(Role::Viewer) => team.to_vec(),
    }
}

/// The hero as the app ships, per role.
pub fn default_for(role: Option<Role>) -> Layout {
    let s = |big: Pick, a: Pick, b: Pick| Slot { big, rows: [a, b] };
    use Figure::*;
    match role {
        None | Some(Role::Admin) => [
            s(Some(Portfolio), Some(Gain6m), Some(ExpectedInterest)),
            s(Some(LentOut), Some(ActiveLoans), Some(DueNext7)),
            s(Some(InterestEarned), Some(AvailableInterest), Some(CollectedMonth)),
            s(Some(AvailableCapital), Some(Overdue), Some(OverdueLoans)),
        ],
        Some(r) => [
            s(Some(Overdue), Some(OverdueLoans), None),
            s(Some(DueNext7), Some(ActiveLoans), None),
            s(Some(Receivable), None, None),
            if r == Role::Collector { s(Some(ToConfirm), None, None) } else { s(None, None, None) },
        ],
    }
}

/// A saved spot: "none", or a figure this role may see (`None` when it
/// isn't, so the spot falls back to its default).
fn read_pick(v: Option<&Value>, ok: &[Figure], big: bool) -> Option<Pick> {
    match v?.as_str()? {
        "none" => Some(None),
        k => Figure::from_key(k).filter(|f| ok.contains(f) && !(big && f.row_only())).map(Some),
    }
}

/// Reads a saved hero (the JSON text). Anything unknown or not allowed for
/// this role falls back to that spot's default, so a role change never
/// shows a total.
pub fn parse(text: &str, role: Option<Role>) -> Layout {
    let mut out = default_for(role);
    let Ok(Value::Array(raw)) = serde_json::from_str::<Value>(text) else { return out };
    let ok = allowed(role);
    for (slot, saved) in out.iter_mut().zip(raw.iter()) {
        if !saved.is_object() {
            continue;
        }
        if let Some(p) = read_pick(saved.get("big"), &ok, true) {
            slot.big = p;
        }
        let rows = saved.get("rows").and_then(Value::as_array);
        for (i, row) in slot.rows.iter_mut().enumerate() {
            if let Some(p) = read_pick(rows.and_then(|r| r.get(i)), &ok, false) {
                *row = p;
            }
        }
    }
    out
}

/// The JSON text saved to the account (the web's shape).
pub fn to_text(l: &Layout) -> String {
    let key = |p: Pick| p.map_or("none", Figure::key);
    let slots: Vec<Value> = l.iter().map(|s| json!({ "big": key(s.big), "rows": [key(s.rows[0]), key(s.rows[1])] })).collect();
    Value::Array(slots).to_string()
}

/// The places to draw, empty ones dropped: the big figure and the rows
/// that aren't empty.
pub fn shown(l: &Layout) -> Vec<(Pick, Vec<Figure>)> {
    l.iter()
        .map(|s| (s.big, s.rows.iter().flatten().copied().collect::<Vec<_>>()))
        .filter(|(big, rows)| big.is_some() || !rows.is_empty())
        .collect()
}

/// Changes one spot of one place.
pub fn set(mut l: Layout, place: usize, spot: Spot, value: Pick) -> Layout {
    if let Some(s) = l.get_mut(place) {
        match spot {
            Spot::Big => s.big = value,
            Spot::Row(i) if i < 2 => s.rows[i] = value,
            Spot::Row(_) => {}
        }
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_in_the_webs_shape() {
        let l = default_for(Some(Role::Admin));
        let text = to_text(&l);
        assert!(text.starts_with(r#"[{"big":"portfolio","rows":["gain6m","expectedInterest"]}"#), "{text}");
        assert_eq!(parse(&text, Some(Role::Admin)), l);
        let viewer = to_text(&default_for(Some(Role::Viewer)));
        assert!(viewer.contains(r#"{"big":"none","rows":["none","none"]}"#));
    }

    #[test]
    fn a_role_never_gets_a_total_it_may_not_see() {
        let mine = set(default_for(Some(Role::Admin)), 0, Spot::Row(1), Some(Figure::Overdue));
        // Read as a viewer: the totals fall back to the viewer's defaults,
        // the overdue row (allowed) stays.
        let as_viewer = parse(&to_text(&mine), Some(Role::Viewer));
        let ok = allowed(Some(Role::Viewer));
        for s in as_viewer {
            for f in [s.big].into_iter().chain(s.rows).flatten() {
                assert!(ok.contains(&f), "{f:?}");
            }
        }
        assert_eq!(as_viewer[0].rows[1], Some(Figure::Overdue));
        assert!(!allowed(Some(Role::Admin)).contains(&Figure::ToConfirm));
        assert!(allowed(Some(Role::Collector)).contains(&Figure::ToConfirm));
    }

    #[test]
    fn bad_text_falls_back_and_gain_is_never_big() {
        assert_eq!(parse("not json", None), default_for(None));
        assert_eq!(parse("{}", None), default_for(None));
        let l = parse(r#"[{"big":"gain6m","rows":["nonsense","none"]}]"#, None);
        assert_eq!(l[0].big, Some(Figure::Portfolio));
        assert_eq!(l[0].rows, [Some(Figure::Gain6m), None]);
        assert_eq!(l[1], default_for(None)[1]);
    }

    #[test]
    fn empty_places_are_not_shown() {
        let l = default_for(Some(Role::Viewer));
        let shown = shown(&l);
        assert_eq!(shown.len(), 3);
        assert_eq!(shown[2], (Some(Figure::Receivable), vec![]));
    }
}
