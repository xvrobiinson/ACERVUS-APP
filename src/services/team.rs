//! The Team section (roadmap 58, approved by the owner 2026-11-11:
//! https://claude.ai/artifact/JbAJr5H8wxLFWmujp8SQDe): the order of the
//! list, how "online" and "last seen" read, and what each collector brought
//! in today. The list comes from the cloud (`team:list`, convex/team.ts);
//! the web's twin is src/lib/team.ts.

use chrono::{DateTime, Local, NaiveDate, TimeZone as _};

use crate::sync::wire::{Role, Snapshot, TeamMember, TeamStatus};

/// How often an open app says "I'm here" (convex/team.ts BEAT_MS).
pub const BEAT_SECS: u64 = 60;
/// The Team column shows from this window width; below it, the people button.
pub const COLUMN_FROM: f32 = 1600.;
/// The column's width with its margin, open and folded.
pub const COLUMN_W: f32 = 336.;
pub const FOLDED_W: f32 = 92.;

fn rank(r: Role) -> u8 {
    match r {
        Role::Owner => 0,
        Role::Admin => 1,
        Role::Manager => 2,
        Role::Accountant => 3,
        Role::Collector => 4,
    }
}

/// The person here first, then who is online, the Owner, Admins and
/// Managers before Accountants and Collectors, then the most recently seen,
/// then by name (the web's `teamOrder`).
pub fn order(members: &[TeamMember]) -> Vec<TeamMember> {
    let online = |m: &TeamMember| m.status.as_ref().is_some_and(|s| s.online);
    let seen = |m: &TeamMember| m.status.as_ref().map_or(0., |s| s.seen_at);
    let mut out = members.to_vec();
    out.sort_by(|a, b| {
        b.me.cmp(&a.me)
            .then(online(b).cmp(&online(a)))
            .then((rank(a.role) > 2).cmp(&(rank(b.role) > 2)))
            .then(if online(a) { std::cmp::Ordering::Equal } else { seen(b).total_cmp(&seen(a)) })
            .then(rank(a.role).cmp(&rank(b.role)))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// A business with more than one person has a Team section (decision 2).
pub fn has_team(snap: &Snapshot) -> bool {
    snap.people.iter().filter(|p| p.active).count() > 1
}

/// When someone was last there, as the list says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    Online,
    JustNow,
    Minutes(i64),
    /// Earlier today, at this time.
    Today(String),
    Yesterday(String),
    On(NaiveDate),
}

pub fn seen(status: &TeamStatus, now: DateTime<Local>) -> Seen {
    if status.online {
        return Seen::Online;
    }
    let Some(at) = Local.timestamp_millis_opt(status.seen_at as i64).single() else { return Seen::JustNow };
    let mins = (now - at).num_minutes();
    if mins < 1 {
        return Seen::JustNow;
    }
    if mins < 60 {
        return Seen::Minutes(mins);
    }
    let time = at.format("%H:%M").to_string();
    match (now.date_naive() - at.date_naive()).num_days() {
        0 => Seen::Today(time),
        1 => Seen::Yesterday(time),
        _ => Seen::On(at.date_naive()),
    }
}

/// What `user_id` brought in on `today`: payments they recorded (or
/// collected, once confirmed) and their collected payments still waiting.
pub fn collected_today(snap: &Snapshot, user_id: &str, today: NaiveDate) -> (usize, i64) {
    let confirmed =
        |record_no: i64| snap.pending.iter().find(|p| p.status == "confirmed" && p.record_no == record_no).map(|p| p.collected_by.as_str());
    let mut count = 0;
    let mut amount = 0;
    for p in &snap.payments {
        if p.paid_on.0 == today && confirmed(p.record_no).unwrap_or(p.created_by.as_str()) == user_id {
            count += 1;
            amount += p.amount;
        }
    }
    for p in &snap.pending {
        if p.status == "waiting" && p.paid_on.0 == today && p.collected_by == user_id {
            count += 1;
            amount += p.amount;
        }
    }
    (count, amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, role: Role, status: Option<(bool, f64)>, me: bool) -> TeamMember {
        TeamMember {
            user_id: name.into(),
            name: name.into(),
            role,
            me,
            photo_url: None,
            phone: String::new(),
            status: status.map(|(online, seen_at)| TeamStatus { online, seen_at, device: "pc".into() }),
        }
    }

    #[test]
    fn the_list_runs_like_the_web() {
        let list = order(&[
            member("cora", Role::Collector, Some((false, 500.)), false),
            member("max", Role::Manager, Some((false, 100.)), false),
            member("ana", Role::Collector, Some((true, 50.)), false),
            member("me", Role::Admin, Some((true, 1.)), true),
            member("owen", Role::Owner, Some((true, 70.)), false),
            member("hid", Role::Accountant, None, false),
            member("cid", Role::Collector, Some((false, 900.)), false),
        ]);
        let names: Vec<_> = list.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["me", "owen", "ana", "max", "cid", "cora", "hid"]);
    }

    #[test]
    fn last_seen_reads_in_plain_words() {
        let now = Local.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let ago = |min: i64| TeamStatus {
            online: false,
            seen_at: (now - chrono::Duration::minutes(min)).timestamp_millis() as f64,
            device: "pc".into(),
        };
        assert_eq!(seen(&TeamStatus { online: true, ..ago(5) }, now), Seen::Online);
        assert_eq!(seen(&ago(0), now), Seen::JustNow);
        assert_eq!(seen(&ago(12), now), Seen::Minutes(12));
        assert_eq!(seen(&ago(5 * 60), now), Seen::Today("10:00".into()));
        assert_eq!(seen(&ago(20 * 60), now), Seen::Yesterday("19:00".into()));
        assert_eq!(seen(&ago(4 * 24 * 60), now), Seen::On(NaiveDate::from_ymd_opt(2026, 11, 7).unwrap()));
    }
}
