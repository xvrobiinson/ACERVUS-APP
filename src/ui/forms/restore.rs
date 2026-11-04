//! Restoring a backup of a cloud business: the backup's books replace the
//! business's in the cloud, for every phone and PC (admins only). Before the
//! name is typed the dialog says what it means: what the books hold now and
//! in the backup, that everything recorded after the backup is lost, and
//! what stays (the team, invitations, settings, logo, each client's
//! collector and ID photo). A copy of the books as they are now goes to the
//! backups folder first, and the cloud checks the backup like an import:
//! if anything doesn't add up, nothing changes.

use std::path::PathBuf;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use super::{danger_footer, error_banner, field, value};
use crate::app::VzApp;
use crate::i18n::t;
use crate::sync::import::{self, Counts, Prepared};
use crate::sync::outbox::Outcome;
use crate::sync::wire::Role;
use crate::tr;
use crate::ui::theme::{RADIUS_CONTROL, Typeset as _, palette, space, text};

pub struct RestoreForm {
    app: WeakEntity<VzApp>,
    source: PathBuf,
    business_id: String,
    name: String,
    /// When the backup was made, as shown and as the cloud's history keeps it.
    made: (String, String),
    now: Counts,
    backup: Counts,
    /// Why this backup can't be restored here, if it can't.
    problem: Option<String>,
    pub confirm: Entity<InputState>,
    busy: bool,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

/// When `path` was last written, in local time.
fn made_on(path: &std::path::Path) -> Option<chrono::DateTime<chrono::Local>> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().map(chrono::DateTime::from)
}

impl RestoreForm {
    pub fn new(
        app: WeakEntity<VzApp>,
        source: PathBuf,
        business_id: String,
        name: String,
        now: Counts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let confirm = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![cx.subscribe_in(&confirm, window, |_, _, ev: &InputEvent, _, cx| {
            if let InputEvent::Change = ev {
                cx.notify()
            }
        })];
        let made =
            made_on(&source).map_or_else(Default::default, |d| (d.format("%Y-%m-%d %H:%M").to_string(), d.format("%Y-%m-%d").to_string()));
        let problem = Self::problem(&source, &business_id);
        Self {
            app,
            backup: import::counts(&source),
            source,
            business_id,
            name,
            made,
            now,
            problem,
            confirm,
            busy: false,
            error: None,
            _subs: subs,
        }
    }

    /// Only an intact backup of this business, made where the whole of it
    /// was (an admin's PC, or books from before the cloud), can come back.
    fn problem(source: &std::path::Path, business_id: &str) -> Option<String> {
        if let Err(e) = crate::services::Store::inspect_backup(source) {
            return Some(tr!("This file can't be restored: {e:#}", "Este archivo no se puede restaurar: {e:#}"));
        }
        let link = rusqlite::Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()
            .and_then(|c| crate::sync::replica::link(&c).ok().flatten());
        match link {
            Some(l) if l.business_id != business_id => {
                let other = l.business_name;
                Some(tr!(
                    "This backup is of another business, {other}. Open that business to restore it.",
                    "Esta copia es de otro negocio, {other}. Abre ese negocio para restaurarla."
                ))
            }
            Some(l) if l.role != Role::Admin => Some(
                t(
                    "This backup comes from a collector's or viewer's PC, which holds only part of the books. Use a backup made on an admin's PC.",
                    "Esta copia viene de la PC de un cobrador o de alguien de solo lectura, que tiene solo parte de los registros. Usa una copia hecha en la PC de un administrador.",
                )
                .into(),
            ),
            _ => None,
        }
    }

    fn confirmed(&self, cx: &App) -> bool {
        value(&self.confirm, cx).trim() == self.name
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.problem.is_some() || !self.confirmed(cx) {
            return;
        }
        // First a copy of the books as they are now, then the backup is read.
        let ready = self.app.update(cx, |app, _| {
            let dir = app.store.backups_dir();
            let safety = dir.join(format!("before-restore-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S")));
            let today = app.store.cloud_link().ok().flatten().map_or_else(crate::utils::dates::today, |l| l.today);
            std::fs::create_dir_all(&dir).map_err(anyhow::Error::from).and_then(|()| app.store.backup_to(&safety)).map(|()| (safety, today))
        });
        let (safety, today) = match ready {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                self.error = Some(tr!(
                    "Nothing was restored: a copy of the books as they are now couldn't be saved first ({e:#}).",
                    "No se restauró nada: no se pudo guardar primero una copia de los registros actuales ({e:#})."
                ));
                return cx.notify();
            }
            Err(_) => return,
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        let source = self.source.clone();
        let prepared = cx.background_spawn(async move { import::prepare(&source, "UTC", today) });
        let (business, name, date) = (self.business_id.clone(), self.name.clone(), self.made.1.clone());
        cx.spawn_in(window, async move |this, cx| {
            let prepared: Prepared = match prepared.await {
                Ok(p) => p,
                Err(e) => {
                    log::error!("backup not read for the cloud: {e:#}");
                    let _ = this.update(cx, |form, cx| {
                        form.busy = false;
                        form.error = Some(tr!("This backup couldn't be read: {e}", "No se pudo leer esta copia: {e}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let mut args = prepared.args;
            args["businessId"] = json!(business);
            args["confirmName"] = json!(name);
            args["backupDate"] = json!(date);
            let answer = this
                .update(cx, |form, cx| form.app.read_with(cx, |app, _| app.cloud_call("imports:restoreBook", args)))
                .ok()
                .and_then(Result::ok)
                .flatten();
            let outcome = match answer {
                Some(a) => a.await.unwrap_or(Outcome::NoAnswer),
                None => Outcome::NotSent,
            };
            let _ = this.update_in(cx, |form, window, cx| form.answered(outcome, safety, window, cx));
        })
        .detach();
    }

    fn answered(&mut self, outcome: Outcome, safety: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        if matches!(outcome, Outcome::Accepted(_)) {
            window.close_dialog(cx);
            let (date, saved) = (self.made.0.clone(), safety.display().to_string());
            let _ = self.app.update(cx, |app, cx| {
                app.notify_success(
                    tr!(
                        "Restored. Every phone and PC shows the books from {date} at its next sync. The books as they were are saved in {saved}.",
                        "Restaurado. Cada teléfono y PC muestra los registros del {date} en su próxima sincronización. Los registros como estaban se guardaron en {saved}."
                    ),
                    window,
                    cx,
                );
                app.pump(window, cx);
            });
            return;
        }
        self.error = Some(match &outcome {
            Outcome::Refused(e) if e["code"] == "import_mismatch" => {
                log::error!("restore refused, figures differ: {}", e["differences"]);
                t(
                    "The cloud worked out some of this backup's figures differently, so nothing was restored. The business is unchanged.",
                    "La nube calculó algunas cifras de esta copia de otra forma, así que no se restauró nada. El negocio sigue igual.",
                )
                .to_string()
            }
            Outcome::Refused(e) if e["code"] == "import_stale" => t(
                "The day changed while restoring. Nothing was restored; try again.",
                "El día cambió mientras se restauraba. No se restauró nada; inténtalo de nuevo.",
            )
            .to_string(),
            _ => self.app.read_with(cx, |app, _| app.call_failed(&outcome)).unwrap_or_default(),
        });
        cx.notify();
    }
}

impl Render for RestoreForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let name = self.name.clone();
        let date = self.made.0.clone();
        let line = |label: String, c: Counts| {
            let Counts { clients, loans, payments } = c;
            h_flex().justify_between().gap(px(space::M)).child(div().text_color(p.muted).child(label)).child(
                div().text_color(p.text).child(tr!(
                    "{clients} clients · {loans} loans · {payments} payments",
                    "{clients} clientes · {loans} préstamos · {payments} pagos"
                )),
            )
        };
        let compare = v_flex()
            .gap(px(space::XS))
            .p(px(space::M))
            .rounded(px(RADIUS_CONTROL))
            .bg(p.raised)
            .typeset(text::BODY)
            .child(line(t("Now", "Ahora").to_string(), self.now))
            .child(line(tr!("Backup of {date}", "Copia del {date}"), self.backup));
        let ready = self.problem.is_none();
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.problem.clone().or_else(|| self.error.clone())))
            .child(div().typeset(text::BODY).text_color(p.text).child(tr!(
                "{name}'s books go back to this backup, on every phone and PC. Everything recorded after it is lost: clients, loans, payments and edits.",
                "Los registros de {name} vuelven a esta copia, en cada teléfono y PC. Todo lo registrado después se pierde: clientes, préstamos, pagos y cambios."
            )))
            .child(compare)
            .child(div().typeset(text::BODY).text_color(p.muted).child(t(
                "The team, invitations, settings and logo stay, and each client keeps its collector and ID photo. A copy of the books as they are now is saved in the backups folder first, and the cloud checks the backup before anything changes.",
                "El equipo, las invitaciones, la configuración y el logo se quedan, y cada cliente conserva su cobrador y su foto. Primero se guarda en la carpeta de copias una copia de los registros como están ahora, y la nube revisa la copia antes de cambiar nada.",
            )))
            .when(ready, |d| {
                d.child(field(&p, &tr!("Type “{name}” to confirm", "Escribe “{name}” para confirmar"), Input::new(&self.confirm), None, None))
            })
            .child(danger_footer(
                "restore-cloud",
                if self.busy { t("Restoring…", "Restaurando…") } else { t("Restore for everyone", "Restaurar para todos") },
                ready && self.confirmed(cx) && !self.busy,
                cx.listener(|this, _, w, cx| this.submit(w, cx)),
            ))
    }
}

#[cfg(test)]
mod tests {
    use core::prelude::v1::test;

    use super::*;

    /// A backup file whose copy belongs to `business` with `role` (none:
    /// books from before the cloud).
    fn backup(dir: &std::path::Path, business: Option<(&str, &str)>) -> PathBuf {
        let path = dir.join(format!("{}.db", business.map_or("pc", |b| b.0)));
        drop(crate::services::Store::open(path.clone()).unwrap());
        if let Some((id, role)) = business {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute(
                "INSERT INTO sync_state (id, business_id, business_name, timezone, user_id, role, today, applied_at) VALUES (1, ?1, 'Cobros del Este', 'America/Santo_Domingo', 'u1', ?2, '2026-11-04', '2026-11-04T12:00:00Z')",
                rusqlite::params![id, role],
            )
            .unwrap();
        }
        path
    }

    #[test]
    fn only_a_whole_backup_of_this_business_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(RestoreForm::problem(&backup(dir.path(), Some(("b1", "admin"))), "b1"), None);
        assert_eq!(RestoreForm::problem(&backup(dir.path(), None), "b1"), None, "books from before the cloud");
        let other = RestoreForm::problem(&backup(dir.path(), Some(("b2", "admin"))), "b1").unwrap();
        assert!(other.contains("Cobros del Este"), "{other}");
        assert!(RestoreForm::problem(&backup(dir.path(), Some(("b3", "collector"))), "b3").is_some(), "a collector's part of the books");
        let junk = dir.path().join("junk.db");
        std::fs::write(&junk, b"not a database").unwrap();
        assert!(RestoreForm::problem(&junk, "b1").is_some());
    }
}
