//! What the cloud sends: the PWA's `data.snapshot` and `businesses.mine`
//! results, read into plain structs. Field names follow the Convex schema
//! (`convex/schema.ts` in the PWA repo); money is whole cents, rates parts
//! per million, dates "YYYY-MM-DD", timestamps milliseconds.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Convex sends every `v.number()` as a float (`1000000.0`). Whole numbers
/// become integers here so they read straight into `i64` fields.
pub fn normalize(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9.0e15 => Value::from(f as i64),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(normalize).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        other => other,
    }
}

/// A business the signed-in user belongs to.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub business_id: String,
    pub name: String,
    pub role: Role,
    pub timezone: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Collector,
    Viewer,
}

/// What a change needs from the person's role, as the cloud checks it
/// (`convex/lib/access.ts` in the PWA repo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    /// New clients, payments, expenses and income: collectors and admins.
    Record,
    /// Everything else: admins only.
    Manage,
}

impl Role {
    pub fn allows(self, need: Need) -> bool {
        match need {
            Need::Record => self != Role::Viewer,
            Need::Manage => self == Role::Admin,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub business: Business,
    pub me: Me,
    /// "Today" in the business's time zone.
    pub today: String,
    pub settings: Option<Settings>,
    pub clients: Vec<Client>,
    pub loans: Vec<Loan>,
    pub payments: Vec<Payment>,
    pub late_fees: Vec<LateFee>,
    pub capital: Vec<Capital>,
    pub entries: Vec<Entry>,
    pub status_events: Vec<StatusEvent>,
    /// Payments collectors recorded, waiting for an admin (and recently
    /// decided ones). A collector gets only their own.
    #[serde(default)]
    pub pending: Vec<Pending>,
    /// The team: names, and who can be given clients.
    #[serde(default)]
    pub people: Vec<Person>,
}

impl Snapshot {
    /// Whether the business has any collector: without one, every client is
    /// the admins' own and nothing about collectors is shown.
    pub fn has_collectors(&self) -> bool {
        self.people.iter().any(|p| p.active && p.role == Some(Role::Collector))
    }

    /// A person's name ("" when unknown).
    pub fn name_of(&self, user_id: &str) -> &str {
        self.people.iter().find(|p| p.user_id == user_id).map_or("", |p| p.name.as_str())
    }

    /// The payments still waiting for an admin.
    pub fn waiting(&self) -> impl Iterator<Item = &Pending> {
        self.pending.iter().filter(|p| p.status == "waiting")
    }

    /// Who collects from a client (by its record number here), if anyone.
    pub fn collector_of(&self, client_no: i64) -> Option<&str> {
        self.clients.iter().find(|c| c.record_no == client_no).and_then(|c| c.collector_id.as_deref())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Business {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub timezone: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub user_id: String,
    pub role: Role,
    pub name: String,
    /// A collector's switches; everything shows for anyone else.
    #[serde(default)]
    pub view: Option<View>,
}

/// What a collector sees of the clients assigned to them. The cloud leaves
/// out phone, address, ID and notes when off; the rest is hidden on screen.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub amount_lent: bool,
    pub rate: bool,
    pub history: bool,
    pub late_fees: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { amount_lent: true, rate: true, history: true, late_fees: true }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub receipt_name: String,
    pub company_address: String,
    pub company_phone: String,
    pub receipt_company: ReceiptCompany,
    pub receipt_client: ReceiptClient,
    /// Optional parts of payment receipts by cloud key; missing ones are shown.
    #[serde(default)]
    pub receipt_show: BTreeMap<String, bool>,
    /// "full" or "next"; unset is the full schedule.
    #[serde(default)]
    pub receipt_format: Option<String>,
    pub receipt_text: BTreeMap<String, String>,
    pub currency_code: String,
    pub number_style: String,
    pub date_style: String,
    pub language: String,
    pub default_rate: i64,
    pub default_frequency: String,
    pub late_fee_rate: i64,
    /// 0.15: new loans' way of charging late fees and interest reinvested.
    #[serde(default)]
    pub late_fee_mode: Option<String>,
    #[serde(default)]
    pub default_reinvest: Option<i64>,
    pub receipt_after_payment: bool,
    pub receipt_after_loan: bool,
    pub logo_url: Option<String>,
    pub version: i64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ReceiptCompany {
    pub name: bool,
    pub address: bool,
    pub phone: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptClient {
    pub name: bool,
    pub id_number: bool,
    pub phone: bool,
    pub address: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    #[serde(rename = "_id")]
    pub id: String,
    pub record_no: i64,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub id_number: String,
    pub address: String,
    pub notes: String,
    pub archived_at: Option<i64>,
    pub has_photo: bool,
    pub created_at: i64,
    pub version: i64,
    /// The collector (or admin) who collects from this client; none: nobody yet.
    #[serde(default)]
    pub collector_id: Option<String>,
}

/// A payment a collector recorded (`pendingPayments` in the cloud). It
/// counts for nothing until an admin confirms it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    #[serde(rename = "_id")]
    pub id: String,
    pub loan_id: String,
    pub client_id: String,
    /// Receipt number, kept by the payment once confirmed.
    pub record_no: i64,
    pub paid_on: Day,
    pub amount: i64,
    pub notes: String,
    pub collected_by: String,
    pub collector_name: String,
    /// "waiting", "confirmed" or "rejected".
    pub status: String,
    pub reason: String,
    pub created_at: i64,
    #[serde(default)]
    pub decided_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub user_id: String,
    pub name: String,
    #[serde(default)]
    pub role: Option<Role>,
    #[serde(default = "yes")]
    pub active: bool,
}

fn yes() -> bool {
    true
}

/// A message in the bell (`messages:mine`); the words are put together
/// here, in this PC's language.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    #[serde(rename = "_id")]
    pub id: String,
    /// due_unassigned, assigned, unassigned, collected, confirmed, rejected.
    pub kind: String,
    pub client_names: Vec<String>,
    pub from_name: String,
    #[serde(default)]
    pub amount: Option<i64>,
    #[serde(default)]
    pub due_on: Option<Day>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub pending_id: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub read_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct Installment {
    pub seq: i64,
    pub due: Day,
    pub principal: i64,
    pub interest: i64,
}

/// "YYYY-MM-DD", kept as text until the replica stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day(pub chrono::NaiveDate);

impl<'de> Deserialize<'de> for Day {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").map(Day).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Loan {
    #[serde(rename = "_id")]
    pub id: String,
    pub record_no: i64,
    pub client_id: String,
    pub principal: i64,
    pub rate: i64,
    pub interest_period: String,
    pub frequency: String,
    pub method: String,
    pub installments: Option<i64>,
    pub schedule_edited: bool,
    pub schedule: Vec<Installment>,
    pub late_fee_rate: i64,
    /// 0.15: its way of charging late fees and interest reinvested.
    #[serde(default)]
    pub late_fee_mode: Option<String>,
    #[serde(default)]
    pub reinvest: Vec<ReinvestChoice>,
    pub start_date: Day,
    pub status: String,
    pub status_date: Option<Day>,
    pub notes: String,
    pub created_at: i64,
    pub version: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct ReinvestChoice {
    pub from: Day,
    pub percent: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct Applied {
    pub seq: i64,
    pub principal: i64,
    pub interest: i64,
    pub fee: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct Target {
    pub seq: i64,
    pub amount: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    #[serde(rename = "_id")]
    pub id: String,
    pub loan_id: String,
    pub record_no: i64,
    pub paid_on: Day,
    pub amount: i64,
    pub notes: String,
    pub mode: String,
    pub target_seq: Option<i64>,
    pub targets: Option<Vec<Target>>,
    pub principal: i64,
    pub interest: i64,
    pub fee: i64,
    pub applied: Vec<Applied>,
    /// How it was paid; none on older payments.
    #[serde(default)]
    pub paid_with: Option<String>,
    /// Who recorded it (a user id), for the collections report.
    #[serde(default)]
    pub created_by: String,
    pub created_at: i64,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LateFee {
    #[serde(rename = "_id")]
    pub id: String,
    pub loan_id: String,
    pub seq: i64,
    pub assessed_on: Day,
    pub base_amount: i64,
    pub rate: i64,
    pub amount: i64,
    pub waived_at: Option<i64>,
    pub waive_reason: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capital {
    #[serde(rename = "_id")]
    pub id: String,
    pub record_no: i64,
    pub kind: String,
    pub amount: i64,
    pub occurred_on: Day,
    pub notes: String,
    pub created_at: i64,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(rename = "_id")]
    pub id: String,
    pub record_no: i64,
    pub kind: String,
    pub account: String,
    pub amount: i64,
    pub occurred_on: Day,
    pub description: String,
    pub created_at: i64,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub loan_id: String,
    pub status: String,
    pub occurred_on: Day,
    pub note: String,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn whole_floats_become_integers() {
        let v = normalize(json!({ "a": 1000000.0, "b": [2.0, 2.5], "c": "3" }));
        assert_eq!(v, json!({ "a": 1000000, "b": [2, 2.5], "c": "3" }));
        assert!(v["a"].is_i64());
    }

    #[test]
    fn a_payment_reads_from_convex_json() {
        let p: Payment = serde_json::from_value(normalize(json!({
            "_id": "k1", "_creationTime": 1.0, "businessId": "b1", "loanId": "l1", "recordNo": 7.0,
            "paidOn": "2026-10-01", "amount": 300000.0, "notes": "", "mode": "spread",
            "targets": [{ "seq": 1.0, "amount": 300000.0 }], "feeInput": 0.0,
            "principal": 200000.0, "interest": 100000.0, "fee": 0.0,
            "applied": [{ "seq": 1.0, "principal": 200000.0, "interest": 100000.0, "fee": 0.0 }],
            "recordedOffline": false, "createdAt": 1.0, "createdBy": "u", "updatedAt": 1.0, "version": 3.0
        })))
        .unwrap();
        assert_eq!((p.record_no, p.amount, p.version), (7, 300000, 3));
        assert_eq!(p.paid_on.0, chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        assert_eq!(p.targets.unwrap()[0], Target { seq: 1, amount: 300000 });
        assert_eq!(p.target_seq, None);
    }

    #[test]
    fn collectors_read_from_convex_json_and_older_clouds_still_read() {
        let c: Client = serde_json::from_value(normalize(json!({
            "_id": "c1", "recordNo": 3.0, "fullName": "María", "phone": "", "email": "", "idNumber": "", "address": "",
            "notes": "", "archivedAt": null, "hasPhoto": false, "createdAt": 1.0, "version": 1.0, "collectorId": "user_ana"
        })))
        .unwrap();
        assert_eq!(c.collector_id.as_deref(), Some("user_ana"));
        let p: Pending = serde_json::from_value(normalize(json!({
            "_id": "p1", "businessId": "b", "loanId": "l1", "clientId": "c1", "recordNo": 18.0, "paidOn": "2026-10-31",
            "amount": 250000.0, "fee": 0.0, "notes": "", "mode": "auto", "collectedBy": "user_ana", "collectorName": "Ana",
            "status": "waiting", "reason": "", "recordedOffline": false, "createdAt": 1.0
        })))
        .unwrap();
        assert_eq!((p.record_no, p.amount, p.status.as_str(), p.decided_at), (18, 250000, "waiting", None));
        // A person from a cloud that didn't send roles yet.
        let who: Person = serde_json::from_value(json!({ "userId": "u", "name": "Ana", "email": "" })).unwrap();
        assert_eq!((who.role, who.active), (None, true));
        let m: Message = serde_json::from_value(normalize(json!({
            "_id": "m1", "businessId": "b", "toUserId": "u", "kind": "collected", "clientNames": ["María"], "fromName": "Ana",
            "amount": 250000.0, "pendingId": "p1", "createdAt": 5.0
        })))
        .unwrap();
        assert_eq!((m.kind.as_str(), m.amount, m.read_at), ("collected", Some(250000), None));
    }
}
