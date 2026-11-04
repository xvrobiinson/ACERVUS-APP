//! The change history in plain words: what was edited or deleted, which
//! record it was (the loan's client by name), and each field by its name in
//! the app with values written the way the app shows them. The app's own
//! late fee updates after a change fold into one entry per loan. Matches
//! the phone app's History (src/lib/history-text.ts there).

use chrono::{DateTime, Local, Utc};

use crate::database::repositories::changes::{Action, Change, Entity, FieldChange};
use crate::domain::loans::{Frequency, Method};
use crate::domain::money::Rate;
use crate::domain::portfolio::{Account, CapitalKind, EntryKind};
use crate::i18n::t;
use crate::services::Snapshot;
use crate::tr;
use crate::utils::dates::parse_iso;

/// One line under a change: `old → new` for an edit, `value` alone otherwise.
pub struct Line {
    pub label: String,
    pub old: Option<String>,
    pub value: String,
}

pub struct Described {
    pub title: String,
    pub lines: Vec<Line>,
    pub reason: String,
    /// When the change was made, in local time.
    pub when: String,
    pub deleted: bool,
}

/// A row of the history: one change, or a run of the app's own late fee
/// updates on one loan.
pub enum Item<'a> {
    One(&'a Change),
    Fees(Vec<&'a Change>),
}

impl Item<'_> {
    pub fn first(&self) -> &Change {
        match self {
            Item::One(c) => c,
            Item::Fees(list) => list[0],
        }
    }
}

fn field_label(entity: Entity, field: &str) -> String {
    match field {
        "principal" if entity == Entity::Payment => t("Capital", "Capital"),
        "client" => t("Client", "Cliente"),
        "principal" => t("Amount lent", "Monto prestado"),
        "rate" => t("Interest rate", "Tasa de interés"),
        "frequency" => t("Payment cycle", "Frecuencia de pago"),
        "start_date" => t("Loan date", "Fecha del préstamo"),
        "amount" => t("Amount", "Monto"),
        "fee" => t("Late fee", "Mora"),
        "interest" => t("Interest", "Interés"),
        "paid_on" => t("Paid on", "Pagado el"),
        "paid_with" => t("Paid with", "Forma de pago"),
        "occurred_on" => t("Date", "Fecha"),
        "kind" => t("Type", "Tipo"),
        "account" => t("Account", "Cuenta"),
        "description" => t("Description", "Descripción"),
        "full_name" => t("Name", "Nombre"),
        "phone" => t("Phone", "Teléfono"),
        "email" => t("Email", "Correo"),
        "id_number" => t("ID number", "Identificación"),
        "address" => t("Address", "Dirección"),
        "photo" => t("Photo", "Foto"),
        "payments" => t("Payments recorded", "Pagos registrados"),
        "received" => t("Received", "Recibido"),
        "interest_period" => t("Interest charged", "Interés cobrado"),
        "method" => t("Loan type", "Tipo de préstamo"),
        "installments" => t("Installments", "Cuotas"),
        "late_fee_rate" => t("Late fee rate", "Tasa de mora"),
        "schedule" => t("Payment plan", "Plan de pagos"),
        "schedule_edited" => t("Plan changed by hand", "Plan cambiado a mano"),
        "allocation" => t("Applied to", "Aplicado a"),
        "fee_base" => t("Overdue amount it was charged on", "Monto vencido sobre el que se cobró"),
        "waived" => t("Waived", "Perdonado"),
        "notes" => t("Notes", "Notas"),
        "backup_restored" => t("Backup restored, made on", "Copia restaurada, hecha el"),
        // A field from a newer version: its own name, in words.
        other => {
            let w = other.replace('_', " ");
            let mut c = w.chars();
            return c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
        }
    }
    .to_string()
}

fn field_value(field: &str, raw: &str, snap: &Snapshot) -> String {
    if raw.is_empty() {
        return "—".into();
    }
    let money = snap.settings.money();
    let cents = |v: &str| v.parse().map(|c| money.format(c)).unwrap_or_else(|_| v.into());
    match field {
        "principal" | "amount" | "fee" | "interest" | "received" | "fee_base" => cents(raw),
        "rate" | "late_fee_rate" => raw.parse().map(|ppm| Rate(ppm).percent_label()).unwrap_or_else(|_| raw.into()),
        "frequency" | "interest_period" => Frequency::from_key(raw).map(|f| f.label().to_string()).unwrap_or_else(|| raw.into()),
        "method" => Method::from_key(raw).map(|m| m.short_label().to_string()).unwrap_or_else(|| raw.into()),
        "paid_with" => {
            crate::database::repositories::payments::PaidWith::from_key(raw).map_or_else(|| raw.into(), |w| w.label().to_string())
        }
        "schedule" => match raw.split_once('|') {
            Some((n, total)) => {
                let total = cents(total);
                tr!("{n} installments · total {total}", "{n} cuotas · total {total}")
            }
            None => raw.into(),
        },
        "schedule_edited" | "waived" => if raw == "true" { t("Yes", "Sí") } else { t("No", "No") }.into(),
        "allocation" => match raw.split_once(' ') {
            Some(("installment", seq)) => tr!("Installment {seq}", "Cuota {seq}"),
            Some(("spread", parts)) => parts
                .split(',')
                .filter_map(|p| p.split_once(':'))
                .map(|(seq, amount)| {
                    let amount = cents(amount);
                    tr!("Installment {seq}: {amount}", "Cuota {seq}: {amount}")
                })
                .collect::<Vec<_>>()
                .join(" · "),
            Some(("custom", parts)) => {
                let p: Vec<String> = parts.split('/').map(cents).collect();
                match p.as_slice() {
                    [capital, interest, fee] => tr!(
                        "Capital {capital} · Interest {interest} · Late fee {fee}",
                        "Capital {capital} · Interés {interest} · Mora {fee}"
                    ),
                    _ => raw.into(),
                }
            }
            _ => t("Oldest installment first", "La cuota más antigua primero").into(),
        },
        "start_date" | "paid_on" | "occurred_on" => {
            parse_iso(raw).map(|d| snap.settings.date_style.format(d)).unwrap_or_else(|| raw.into())
        }
        "client" => raw
            .parse::<i64>()
            .ok()
            .and_then(|id| snap.client(id).map(|c| c.full_name.clone()))
            .unwrap_or_else(|| t("A client no longer in the books", "Un cliente que ya no está").into()),
        "kind" => {
            CapitalKind::from_key(raw).map(|k| k.label()).or_else(|| EntryKind::from_key(raw).map(|k| k.label())).unwrap_or(raw).to_string()
        }
        "account" => Account::from_key(raw).map(|a| a.label().to_string()).unwrap_or_else(|| raw.into()),
        "photo" => match raw {
            "replaced" => t("New photo", "Foto nueva").into(),
            _ => t("On file", "Guardada").into(),
        },
        _ => raw.into(),
    }
}

/// The kind of a cash entry: from the entry if it still exists, else from
/// what was recorded when it was deleted.
fn entry_kind(c: &Change, snap: &Snapshot) -> Option<EntryKind> {
    snap.entry(c.entity_id)
        .map(|e| e.kind)
        .or_else(|| c.fields.iter().find(|f| f.field == "kind").and_then(|f| EntryKind::from_key(&f.old)))
}

/// "Loan #3 · María Rodríguez", or "Loan #3" once the loan is gone.
fn loan_name(id: i64, snap: &Snapshot) -> String {
    match snap.loan(id).and_then(|l| snap.client(l.client_id)) {
        Some(client) => {
            let name = &client.full_name;
            tr!("Loan #{id} · {name}", "Préstamo #{id} · {name}")
        }
        None => tr!("Loan #{id}", "Préstamo #{id}"),
    }
}

/// The app's own notes in the reader's language; a typed reason as it is.
fn reason_text(r: &str) -> String {
    match r {
        "Recalculated after a change" => t("Recalculated after a change", "Recalculado después de un cambio").into(),
        "No longer overdue after a change" => t("No longer overdue after a change", "Ya no está vencido después de un cambio").into(),
        _ => r.into(),
    }
}

fn when(c: &Change, snap: &Snapshot) -> String {
    DateTime::parse_from_rfc3339(&c.created_at)
        .map(|d| {
            let local = d.with_timezone(&Utc).with_timezone(&Local);
            format!("{} {}", snap.settings.date_style.format(local.date_naive()), local.format("%H:%M"))
        })
        .unwrap_or_else(|_| c.created_at.clone())
}

/// A late fee the app worked out again by itself (not one a person waived).
fn automatic_fee(c: &Change) -> bool {
    c.entity == Entity::LateFee && c.fields.iter().all(|f| f.field == "amount" || f.field == "fee_base")
}

/// The history with runs of the app's own late fee updates on one loan
/// folded into one row each.
pub fn fold<'a>(changes: &[&'a Change]) -> Vec<Item<'a>> {
    let mut out: Vec<Item<'a>> = Vec::new();
    for &c in changes {
        if automatic_fee(c)
            && let Some(last) = out.last_mut()
            && automatic_fee(last.first())
            && last.first().loan_id == c.loan_id
        {
            match last {
                Item::Fees(list) => list.push(c),
                Item::One(p) => *last = Item::Fees(vec![*p, c]),
            }
            continue;
        }
        out.push(Item::One(c));
    }
    out
}

/// "1–3, 5": installment numbers, runs shortened.
fn runs(mut n: Vec<i64>) -> String {
    n.sort_unstable();
    n.dedup();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < n.len() {
        let mut j = i;
        while j + 1 < n.len() && n[j + 1] == n[j] + 1 {
            j += 1;
        }
        parts.push(if j > i { format!("{}–{}", n[i], n[j]) } else { n[i].to_string() });
        i = j + 1;
    }
    parts.join(", ")
}

pub fn describe_item(item: &Item<'_>, snap: &Snapshot) -> Described {
    match item {
        Item::One(c) => describe(c, snap),
        Item::Fees(list) => describe_fees(list, snap),
    }
}

/// A folded run of late fee updates: worked out again or dropped, by
/// installment, with the totals.
fn describe_fees(list: &[&Change], snap: &Snapshot) -> Described {
    let money = snap.settings.money();
    let amount = |c: &Change, new: bool| -> i64 {
        c.fields.iter().find(|f| f.field == "amount").and_then(|f| if new { &f.new } else { &f.old }.parse().ok()).unwrap_or(0)
    };
    // Per installment (newest first in `list`): from what it was before the
    // oldest change to what the newest one left.
    let mut seqs: Vec<i64> = list.iter().map(|c| c.entity_id).collect();
    seqs.sort_unstable();
    seqs.dedup();
    let (mut changed, mut dropped) = (Vec::new(), Vec::new());
    let (mut changed_old, mut changed_new, mut dropped_old) = (0, 0, 0);
    for seq in seqs {
        let of: Vec<&&Change> = list.iter().filter(|c| c.entity_id == seq).collect();
        let (newest, oldest) = (of[0], of[of.len() - 1]);
        if newest.action == Action::Delete {
            dropped.push(seq);
            dropped_old += amount(oldest, false);
        } else {
            changed.push(seq);
            changed_old += amount(oldest, false);
            changed_new += amount(newest, true);
        }
    }
    let which = |n: Vec<i64>| {
        if n.len() == 1 {
            let n = n[0];
            tr!("Installment {n}", "Cuota {n}")
        } else {
            let n = runs(n);
            tr!("Installments {n}", "Cuotas {n}")
        }
    };
    let mut lines = Vec::new();
    if !changed.is_empty() {
        let which = which(changed);
        lines.push(Line {
            label: tr!("Recalculated · {which}", "Recalculada · {which}"),
            old: Some(money.format(changed_old)),
            value: money.format(changed_new),
        });
    }
    if !dropped.is_empty() {
        let which = which(dropped);
        lines.push(Line {
            label: tr!("No longer overdue · {which}", "Ya no está vencida · {which}"),
            old: Some(money.format(dropped_old)),
            value: money.format(0),
        });
    }
    let loan = loan_name(list[0].loan_id.unwrap_or_default(), snap);
    Described {
        title: tr!("Late fees · {loan}", "Moras · {loan}"),
        lines,
        reason: t(
            "Worked out again after a change to the loan or its payments",
            "Recalculadas después de un cambio en el préstamo o sus pagos",
        )
        .into(),
        when: when(list[0], snap),
        deleted: false,
    }
}

pub fn describe(c: &Change, snap: &Snapshot) -> Described {
    let edited = c.action == Action::Update;
    let id = c.entity_id;
    let title = match c.entity {
        Entity::Loan => {
            let loan = loan_name(id, snap);
            if edited { tr!("{loan} edited", "{loan} editado") } else { tr!("{loan} deleted", "{loan} eliminado") }
        }
        Entity::Payment => {
            let loan = loan_name(c.loan_id.unwrap_or_default(), snap);
            if edited {
                tr!("Payment #{id} edited · {loan}", "Pago #{id} editado · {loan}")
            } else {
                tr!("Payment #{id} deleted · {loan}", "Pago #{id} eliminado · {loan}")
            }
        }
        Entity::LateFee => {
            let loan = loan_name(c.loan_id.unwrap_or_default(), snap);
            if edited {
                tr!("Late fee on installment {id} changed · {loan}", "Mora de la cuota {id} cambiada · {loan}")
            } else {
                tr!("Late fee on installment {id} removed · {loan}", "Mora de la cuota {id} eliminada · {loan}")
            }
        }
        Entity::Capital if edited => t("Capital entry edited", "Movimiento de capital editado").into(),
        Entity::Capital => t("Capital entry deleted", "Movimiento de capital eliminado").into(),
        Entity::Entry => {
            let what = entry_kind(c, snap).map_or(t("Movement", "Movimiento"), |k| k.label());
            if edited { tr!("{what} edited", "{what} editado") } else { tr!("{what} deleted", "{what} eliminado") }
        }
        Entity::Client => {
            let name = snap
                .client(id)
                .map(|x| x.full_name.clone())
                .or_else(|| c.fields.iter().find(|f| f.field == "full_name").map(|f| f.old.clone()))
                .unwrap_or_default();
            if edited {
                tr!("Client {name} edited", "Cliente {name} editado")
            } else {
                tr!("Client {name} deleted", "Cliente {name} eliminado")
            }
        }
    };
    let line = |f: &FieldChange| Line {
        label: field_label(c.entity, &f.field),
        old: edited.then(|| field_value(&f.field, &f.old, snap)),
        value: field_value(&f.field, if edited { &f.new } else { &f.old }, snap),
    };
    // A deletion lists what the record held; nothing for empty fields or a
    // part of a payment that was zero.
    let held = |f: &&FieldChange| !f.old.is_empty() && !(c.entity == Entity::Payment && f.old == "0");
    let lines = c.fields.iter().filter(|f| edited || held(f)).map(line).collect();
    Described { title, lines, reason: reason_text(&c.reason), when: when(c, snap), deleted: c.action == Action::Delete }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fee(id: i64, loan: i64, action: Action, old: &str, new: &str) -> Change {
        Change {
            id,
            entity: Entity::LateFee,
            entity_id: id,
            loan_id: Some(loan),
            action,
            fields: vec![FieldChange { field: "amount".into(), old: old.into(), new: new.into() }],
            reason: String::new(),
            created_at: String::new(),
        }
    }

    #[test]
    fn folds_the_apps_own_late_fee_updates_by_loan() {
        let mut waived = fee(9, 1, Action::Update, "false", "true");
        waived.fields[0].field = "waived".into();
        let list = [
            fee(4, 1, Action::Delete, "7417", ""),
            fee(5, 1, Action::Delete, "7417", ""),
            fee(1, 1, Action::Update, "7417", "9375"),
            waived,
            fee(2, 1, Action::Update, "7417", "9375"),
            fee(3, 2, Action::Update, "100", "200"),
        ];
        let refs: Vec<&Change> = list.iter().collect();
        let sizes: Vec<usize> = fold(&refs)
            .iter()
            .map(|i| match i {
                Item::One(_) => 1,
                Item::Fees(l) => l.len(),
            })
            .collect();
        assert_eq!(sizes, vec![3, 1, 1, 1]);
    }

    #[test]
    fn shortens_installment_runs() {
        assert_eq!(runs(vec![5, 1, 2, 3, 8, 7]), "1–3, 5, 7–8");
        assert_eq!(runs(vec![4]), "4");
    }

    #[test]
    fn unknown_fields_keep_their_own_name() {
        assert_eq!(field_label(Entity::Loan, "status_date"), "Status date");
        assert_eq!(field_label(Entity::Loan, "notes"), "Notes");
        assert_eq!(field_label(Entity::Payment, "principal"), "Capital");
    }
}
