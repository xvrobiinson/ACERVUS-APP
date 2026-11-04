//! The books kept on this PC, made ready to become a new cloud business
//! (the cloud's `imports:desktopBook`). The same mapping as the PWA's
//! `scripts/import-desktop.mjs`, which moved the first books by hand: the
//! cloud works every loan out again and refuses the whole import if any
//! figure differs from this PC's.
//!
//! The PC's file is never changed: a copy is made, brought up to today
//! (late fees, paid-off loans) as the app does at launch, and read.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Map, Value, json};

use crate::services::Store;

/// What goes to the cloud, and what follows once the business exists.
pub struct Prepared {
    /// `imports:desktopBook`'s arguments: `{ book, expected }`.
    pub args: Value,
    /// ID photos: (the client's number here, image type, bytes).
    pub photos: Vec<(i64, String, Vec<u8>)>,
    /// The business logo: (image type, bytes).
    pub logo: Option<(String, Vec<u8>)>,
    pub clients: usize,
    pub loans: usize,
    pub payments: usize,
}

/// How much the books hold, for the offer to move them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub clients: usize,
    pub loans: usize,
    pub payments: usize,
}

/// Counts the clients, loans and (not reversed) payments at `path`.
pub fn counts(path: &Path) -> Counts {
    let Ok(db) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return Counts::default() };
    let n = |sql: &str| db.query_row(sql, [], |r| r.get::<_, i64>(0)).map_or(0, |n| n as usize);
    Counts {
        clients: n("SELECT COUNT(*) FROM clients"),
        loans: n("SELECT COUNT(*) FROM loans"),
        payments: n("SELECT COUNT(*) FROM payments WHERE reversed_at IS NULL"),
    }
}

/// Whether these books hold anything worth moving.
pub fn has_records(path: &Path) -> bool {
    let Ok(db) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return false };
    ["clients", "loans", "capital_transactions", "cash_entries"]
        .iter()
        .any(|table| db.query_row(&format!("SELECT EXISTS (SELECT 1 FROM {table})"), [], |r| r.get::<_, bool>(0)).unwrap_or(false))
}

/// Reads the books at `source` for a new business in `timezone`, with the
/// figures as of `today` (the cloud checks the import on the same day).
pub fn prepare(source: &Path, timezone: &str, today: NaiveDate) -> Result<Prepared> {
    let copy = std::env::temp_dir().join(format!("acervus-move-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&copy);
    let result = prepare_copy(source, &copy, timezone, today);
    for end in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{end}", copy.display()));
    }
    result
}

fn prepare_copy(source: &Path, copy: &Path, timezone: &str, today: NaiveDate) -> Result<Prepared> {
    {
        let src = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).context("opening the books on this PC")?;
        let mut dst = Connection::open(copy).context("no room for a working copy")?;
        rusqlite::backup::Backup::new(&src, &mut dst)?.run_to_completion(256, std::time::Duration::ZERO, None)?;
    }
    let mut store = Store::open(copy.to_path_buf())?;
    store.refresh(today)?;
    let expected = super::figures::figures(&store.snapshot()?.book, today);
    drop(store);
    let db = Connection::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    read(&db, timezone, expected)
}

/// Every row of `sql` as a JSON object (column name → value; files left out).
fn rows(db: &Connection, sql: &str) -> Result<Vec<Map<String, Value>>> {
    let mut stmt = db.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|n| n.to_string()).collect();
    let found = stmt.query_map([], |r| {
        let mut m = Map::new();
        for (i, name) in names.iter().enumerate() {
            let v = match r.get_ref(i)? {
                ValueRef::Null | ValueRef::Blob(_) => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(x) => json!(x),
                ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
            };
            m.insert(name.clone(), v);
        }
        Ok(m)
    })?;
    Ok(found.collect::<rusqlite::Result<_>>()?)
}

/// A stored time ("2026-10-30T14:05:00Z") in milliseconds, as the cloud keeps it.
fn ms(v: &Value) -> Value {
    v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map_or(Value::Null, |d| json!(d.timestamp_millis()))
}

/// A text column, empty when missing.
fn text(r: &Map<String, Value>, k: &str) -> Value {
    json!(r.get(k).and_then(Value::as_str).unwrap_or_default())
}

fn get(r: &Map<String, Value>, k: &str) -> Value {
    r.get(k).cloned().unwrap_or(Value::Null)
}

/// Drops the keys whose value is null (optional fields the cloud leaves out).
fn compact(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.retain(|_, x| !x.is_null());
    }
    v
}

/// Rows grouped by the row number in `key`, in order.
fn grouped(rows: Vec<Map<String, Value>>, key: &str) -> HashMap<i64, Vec<Map<String, Value>>> {
    let mut m: HashMap<i64, Vec<_>> = HashMap::new();
    for r in rows {
        m.entry(r.get(key).and_then(Value::as_i64).unwrap_or_default()).or_default().push(r);
    }
    m
}

fn read(db: &Connection, timezone: &str, expected: Value) -> Result<Prepared> {
    let settings: HashMap<String, String> = rows(db, "SELECT key, value FROM app_settings")?
        .into_iter()
        .filter_map(|r| Some((r.get("key")?.as_str()?.to_string(), r.get("value")?.as_str()?.to_string())))
        .collect();
    let set = |k: &str| settings.get(k).map(String::as_str);
    let flag = |k: &str| matches!(set(k).unwrap_or("1"), "1" | "true");
    let number = |k: &str, d: f64| set(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let name = set("business_name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Mi negocio").to_string();
    let receipt_text: Map<String, Value> = settings
        .iter()
        .filter(|(k, v)| {
            let k = k.as_str();
            (k.starts_with("receipt_rules_")
                || ["receipt_client_sign", "receipt_lender_sign", "receipt_payment_footer", "receipt_loan_footer"].contains(&k))
                && !v.trim().is_empty()
        })
        .map(|(k, v)| (k.clone(), json!(v)))
        .collect();

    let schedules = grouped(rows(db, "SELECT * FROM loan_installments ORDER BY loan_id, seq")?, "loan_id");
    let targets = grouped(rows(db, "SELECT * FROM payment_targets ORDER BY payment_id, seq")?, "payment_id");
    let applied = grouped(rows(db, "SELECT * FROM installment_allocations ORDER BY payment_id, seq")?, "payment_id");
    // Reversed payments (an early desktop feature) no longer count anywhere.
    let payments = rows(
        db,
        "SELECT p.*, a.principal, a.interest, a.fee FROM payments p
         JOIN payment_allocations a ON a.payment_id = p.id WHERE p.reversed_at IS NULL ORDER BY p.id",
    )?;

    let clients: Vec<Value> = rows(db, "SELECT * FROM clients ORDER BY id")?
        .iter()
        .map(|c| {
            compact(json!({
                "no": get(c, "id"), "fullName": get(c, "full_name"), "phone": text(c, "phone"), "email": text(c, "email"),
                "idNumber": text(c, "id_number"), "address": text(c, "address"), "notes": get(c, "notes"),
                "createdAt": ms(&get(c, "created_at")), "archivedAt": ms(&get(c, "archived_at")),
            }))
        })
        .collect();
    let loans: Vec<Value> = rows(db, "SELECT * FROM loans ORDER BY id")?
        .iter()
        .map(|l| {
            let id = l.get("id").and_then(Value::as_i64).unwrap_or_default();
            let schedule: Vec<Value> = schedules
                .get(&id)
                .map(|xs| {
                    xs.iter()
                        .map(|x| json!({ "seq": get(x, "seq"), "due": get(x, "due_date"), "principal": get(x, "principal"), "interest": get(x, "interest") }))
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "no": id, "clientNo": get(l, "client_id"), "principal": get(l, "principal"), "rate": get(l, "period_rate"),
                "interestPeriod": get(l, "interest_period"), "frequency": get(l, "frequency"), "method": get(l, "method"),
                "installments": get(l, "installments"), "scheduleEdited": l.get("schedule_edited").and_then(Value::as_i64) == Some(1),
                "schedule": schedule, "lateFeeRate": get(l, "late_fee_rate"), "startDate": get(l, "start_date"),
                "status": get(l, "status"), "statusDate": get(l, "status_date"), "notes": get(l, "notes"),
                "createdAt": ms(&get(l, "created_at")),
            })
        })
        .collect();
    let status_events: Vec<Value> = rows(db, "SELECT * FROM loan_status_events ORDER BY id")?
        .iter()
        .map(|e| {
            json!({ "loanNo": get(e, "loan_id"), "status": get(e, "status"), "occurredOn": get(e, "occurred_on"),
                "note": get(e, "note"), "createdAt": ms(&get(e, "created_at")) })
        })
        .collect();
    let payment_rows: Vec<Value> = payments
        .iter()
        .map(|p| {
            let id = p.get("id").and_then(Value::as_i64).unwrap_or_default();
            let mut v = compact(json!({
                "no": id, "loanNo": get(p, "loan_id"), "paidOn": get(p, "paid_on"), "amount": get(p, "amount"),
                "notes": get(p, "notes"), "mode": get(p, "allocation_mode"), "targetSeq": get(p, "target_seq"),
                "principal": get(p, "principal"), "interest": get(p, "interest"), "fee": get(p, "fee"),
                "createdAt": ms(&get(p, "created_at")),
            }));
            // Notes may be empty text but never missing.
            v["notes"] = get(p, "notes");
            if let Some(ts) = targets.get(&id) {
                v["targets"] = ts.iter().map(|t| json!({ "seq": get(t, "seq"), "amount": get(t, "amount") })).collect();
            }
            v["applied"] = applied
                .get(&id)
                .map(|xs| {
                    xs.iter()
                        .map(|a| json!({ "seq": get(a, "seq"), "principal": get(a, "principal"), "interest": get(a, "interest"), "fee": get(a, "late_fee") }))
                        .collect()
                })
                .unwrap_or_else(|| json!([]));
            v
        })
        .collect();
    let late_fees: Vec<Value> = rows(db, "SELECT * FROM late_fees ORDER BY loan_id, seq")?
        .iter()
        .map(|f| {
            let mut v = compact(json!({
                "loanNo": get(f, "loan_id"), "seq": get(f, "seq"), "assessedOn": get(f, "assessed_on"),
                "baseAmount": get(f, "base_amount"), "rate": get(f, "rate"), "amount": get(f, "amount"),
                "waivedAt": ms(&get(f, "waived_at")), "createdAt": ms(&get(f, "created_at")), "updatedAt": ms(&get(f, "updated_at")),
            }));
            v["waiveReason"] = get(f, "waive_reason");
            v
        })
        .collect();
    let capital: Vec<Value> = rows(db, "SELECT * FROM capital_transactions ORDER BY id")?
        .iter()
        .map(|c| {
            json!({ "no": get(c, "id"), "kind": get(c, "kind"), "amount": get(c, "amount"), "occurredOn": get(c, "occurred_on"),
                "notes": get(c, "notes"), "createdAt": ms(&get(c, "created_at")) })
        })
        .collect();
    let entries: Vec<Value> = rows(db, "SELECT * FROM cash_entries ORDER BY id")?
        .iter()
        .map(|e| {
            json!({ "no": get(e, "id"), "kind": get(e, "kind"), "account": get(e, "account"), "amount": get(e, "amount"),
                "occurredOn": get(e, "occurred_on"), "description": get(e, "description"), "createdAt": ms(&get(e, "created_at")) })
        })
        .collect();
    let changes: Vec<Value> = rows(db, "SELECT * FROM change_log ORDER BY id")?
        .iter()
        .map(|c| {
            let lines: Vec<Value> = c
                .get("changes")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .split('\n')
                .filter(|l| !l.is_empty())
                .map(|line| {
                    let mut parts = line.split('\t');
                    let field = parts.next().unwrap_or_default();
                    json!({ "field": field, "old": parts.next().unwrap_or_default(), "new": parts.next().unwrap_or_default() })
                })
                .collect();
            let mut v = json!({ "entity": get(c, "entity"), "entityNo": get(c, "entity_id"), "action": get(c, "action"),
                "changes": lines, "reason": get(c, "reason"), "createdAt": ms(&get(c, "created_at")) });
            if let Some(loan) = c.get("loan_id").filter(|x| !x.is_null()) {
                v["loanNo"] = loan.clone();
            }
            v
        })
        .collect();

    let book = json!({
        "business": { "name": name, "timezone": timezone, "language": if set("language") == Some("en") { "en" } else { "es" },
            "currencyCode": set("currency").unwrap_or("DOP") },
        "settings": {
            "receiptName": name,
            "companyAddress": set("company_address").unwrap_or_default(),
            "companyPhone": set("company_phone").unwrap_or_default(),
            "receiptCompany": { "name": flag("receipt_company_name"), "address": flag("receipt_company_address"), "phone": flag("receipt_company_phone") },
            "receiptClient": { "name": flag("receipt_client_name"), "idNumber": flag("receipt_client_id_number"),
                "phone": flag("receipt_client_phone"), "address": flag("receipt_client_address") },
            "receiptText": receipt_text,
            "numberStyle": set("number_style").unwrap_or("comma_dot"),
            "dateStyle": set("date_style").unwrap_or("medium"),
            "defaultRate": number("default_rate", 100_000.),
            "defaultFrequency": set("default_frequency").unwrap_or("monthly"),
            "lateFeeRate": number("late_fee_rate", 50_000.),
            "receiptAfterPayment": flag("receipt_after_payment"),
            "receiptAfterLoan": flag("receipt_after_loan"),
        },
        "clients": clients, "loans": loans, "statusEvents": status_events, "payments": payment_rows,
        "lateFees": late_fees, "capital": capital, "entries": entries, "changes": changes,
    });

    let mut photos = Vec::new();
    let mut stmt = db.prepare("SELECT client_id, mime, data FROM client_photos")?;
    for p in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Vec<u8>>(2)?)))? {
        photos.push(p?);
    }
    let logo = db
        .query_row("SELECT mime, data FROM app_assets WHERE key = 'logo'", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))
        .ok();
    let counts = (count(&book, "clients"), count(&book, "loans"), count(&book, "payments"));
    Ok(Prepared {
        args: json!({ "book": book, "expected": expected }),
        photos,
        logo,
        clients: counts.0,
        loans: counts.1,
        payments: counts.2,
    })
}

/// How many rows of `key` the book holds.
fn count(book: &Value, key: &str) -> usize {
    book[key].as_array().map_or(0, Vec::len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::portfolio::CapitalKind;
    use crate::utils::dates::today;

    #[test]
    fn empty_books_have_nothing_to_move_and_the_file_stays_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("books.db");
        let mut s = Store::open(path.clone()).unwrap();
        assert!(!has_records(&path));
        assert_eq!(counts(&path), Counts::default());

        s.add_capital(CapitalKind::Contribution, 500_000, today(), "Ahorros", today()).unwrap();
        drop(s);
        assert!(has_records(&path));
        let before = std::fs::read(&path).unwrap();
        let p = prepare(&path, "America/Santo_Domingo", today()).unwrap();
        assert_eq!(p.args["book"]["capital"].as_array().map(Vec::len), Some(1));
        assert_eq!(p.args["book"]["capital"][0]["amount"], 500_000);
        assert_eq!(p.args["book"]["business"]["timezone"], "America/Santo_Domingo");
        assert_eq!(p.args["expected"]["asOf"], today().to_string());
        assert_eq!(std::fs::read(&path).unwrap(), before, "the PC's own file is never changed");
    }
}
