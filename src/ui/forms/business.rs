//! Starting a new business in the cloud, and deleting one for everyone after
//! its name is typed (Settings › Cloud sync).

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use super::{danger_footer, error_banner, field, footer, value};
use crate::app::VzApp;
use crate::i18n::{is_es, t};
use crate::sync::outbox::Outcome;
use crate::tr;
use crate::ui::kit::{Dropdown, DropdownItem};
use crate::ui::theme::{Typeset as _, palette, space, text};

/// Sends `mutation` and hands the answer back to the form (`NotSent` when
/// not signed in).
fn send<F: 'static>(
    app: &WeakEntity<VzApp>,
    mutation: &'static str,
    args: serde_json::Value,
    window: &mut Window,
    cx: &mut Context<F>,
    then: impl FnOnce(&mut F, Outcome, &mut Window, &mut Context<F>) + 'static,
) {
    let answer = app.read_with(cx, |app, _| app.cloud_call(mutation, args)).ok().flatten();
    cx.spawn_in(window, async move |this, cx| {
        let outcome = match answer {
            Some(answer) => answer.await.unwrap_or(Outcome::NotSent),
            None => Outcome::NotSent,
        };
        let _ = this.update_in(cx, |form, window, cx| then(form, outcome, window, cx));
    })
    .detach();
}

fn changed<F: 'static>(input: &Entity<InputState>, window: &mut Window, cx: &mut Context<F>) -> Subscription {
    cx.subscribe_in(input, window, |_, _, ev: &InputEvent, _, cx| {
        if let InputEvent::Change = ev {
            cx.notify()
        }
    })
}

pub struct NewBusinessForm {
    app: WeakEntity<VzApp>,
    pub name: Entity<InputState>,
    busy: bool,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

impl NewBusinessForm {
    pub fn new(app: WeakEntity<VzApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![changed(&name, window, cx)];
        Self { app, name, busy: false, error: None, _subs: subs }
    }

    fn typed(&self, cx: &App) -> String {
        value(&self.name, cx).trim().to_string()
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.typed(cx);
        if name.is_empty() || self.busy {
            return;
        }
        let Ok((timezone, currency)) = self.app.read_with(cx, |app, _| (app.new_business_timezone(), app.settings().currency_code.clone()))
        else {
            return;
        };
        let args = json!({ "name": name, "timezone": timezone, "language": if is_es() { "es" } else { "en" }, "currencyCode": currency });
        self.busy = true;
        self.error = None;
        cx.notify();
        send(&self.app, "businesses:create", args, window, cx, move |this, outcome, window, cx| {
            this.busy = false;
            let Outcome::Accepted(id) = &outcome else {
                this.error = this.app.read_with(cx, |app, _| app.call_failed(&outcome)).ok();
                return cx.notify();
            };
            let id = id.as_str().unwrap_or_default().to_string();
            window.close_dialog(cx);
            let _ = this.app.update(cx, |app, cx| {
                app.link_business(&id, window, cx);
                app.notify_success(tr!("{name} was created and is shown on this PC", "{name} se creó y se ve en esta PC"), window, cx);
            });
        });
    }
}

impl Render for NewBusinessForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.error))
            .child(div().typeset(text::BODY).text_color(p.muted).child(t(
                "A new, empty business in the cloud. You'll be its admin and can invite others from the phone app. It starts with this business's currency and time zone.",
                "Un negocio nuevo y vacío en la nube. Serás su administrador y podrás invitar a otros desde la app del teléfono. Empieza con la moneda y la zona horaria de este negocio.",
            )))
            .child(field(&p, t("Business name", "Nombre del negocio"), Input::new(&self.name), None, None))
            .child(footer(
                &p,
                t("Create business", "Crear negocio"),
                !self.typed(cx).is_empty(),
                self.busy,
                cx.listener(|this, _, w, cx| this.submit(w, cx)),
            ))
    }
}

/// The time zones offered here; a business set to another one (in the phone
/// app) keeps it, listed first.
const ZONES: [&str; 16] = [
    "America/Santo_Domingo",
    "America/Curacao",
    "America/Puerto_Rico",
    "America/Port_of_Spain",
    "America/Bogota",
    "America/Caracas",
    "America/Mexico_City",
    "America/Panama",
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "America/Toronto",
    "Europe/London",
    "Europe/Madrid",
    "Europe/Amsterdam",
];

/// The business's name and time zone, in the cloud (Settings › Business).
pub struct BusinessDetailsForm {
    app: WeakEntity<VzApp>,
    business_id: String,
    pub name: Entity<InputState>,
    timezone: String,
    busy: bool,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

impl BusinessDetailsForm {
    pub fn new(
        app: WeakEntity<VzApp>,
        business_id: String,
        name: String,
        timezone: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let subs = vec![changed(&name, window, cx)];
        Self { app, business_id, name, timezone, busy: false, error: None, _subs: subs }
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = value(&self.name, cx).trim().to_string();
        if name.is_empty() || self.busy {
            return;
        }
        let args = json!({ "businessId": self.business_id, "name": name, "timezone": self.timezone });
        self.busy = true;
        self.error = None;
        cx.notify();
        send(&self.app, "businesses:update", args, window, cx, move |this, outcome, window, cx| {
            this.busy = false;
            if !matches!(outcome, Outcome::Accepted(_)) {
                this.error = this.app.read_with(cx, |app, _| app.call_failed(&outcome)).ok();
                return cx.notify();
            }
            window.close_dialog(cx);
            let _ = this.app.update(cx, |app, cx| app.notify_success(t("Business saved", "Negocio guardado"), window, cx));
        });
    }
}

impl Render for BusinessDetailsForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mut zones: Vec<String> = ZONES.iter().map(|z| z.to_string()).collect();
        if !zones.contains(&self.timezone) {
            zones.insert(0, self.timezone.clone());
        }
        let current = self.timezone.clone();
        let menu = Dropdown::new("business-zone").label(current.clone()).items(zones.into_iter().map(|z| {
            let selected = z == current;
            let pick = z.clone();
            DropdownItem::new(
                z,
                cx.listener(move |this, _, _, cx| {
                    this.timezone = pick.clone();
                    cx.notify();
                }),
            )
            .selected(selected)
        }));
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.error))
            .child(field(&p, t("Business name", "Nombre del negocio"), Input::new(&self.name), None, None))
            .child(field(
                &p,
                t("Time zone", "Zona horaria"),
                menu,
                Some(t("“Today” for due dates and late fees.", "El “hoy” de vencimientos y mora.").to_string()),
                None,
            ))
            .child(footer(
                &p,
                t("Save", "Guardar"),
                !value(&self.name, cx).trim().is_empty(),
                self.busy,
                cx.listener(|this, _, w, cx| this.submit(w, cx)),
            ))
    }
}

/// Deleting your own account (Settings › Cloud sync): it waits 7 days and
/// can be cancelled meanwhile, here or in the phone app.
pub struct DeleteAccountForm {
    app: WeakEntity<VzApp>,
    /// What the cloud said: when it deletes, and where the person is the only admin.
    deletes_at: Option<i64>,
    sole_admin_of: Vec<String>,
    loaded: bool,
    busy: bool,
    error: Option<String>,
}

impl DeleteAccountForm {
    pub fn new(app: WeakEntity<VzApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let this = Self { app, deletes_at: None, sole_admin_of: Vec::new(), loaded: false, busy: false, error: None };
        cx.defer_in(window, |form, window, cx| form.load(window, cx));
        this
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let answer = self.app.read_with(cx, |app, _| app.cloud_query("account:status", json!({}))).ok().flatten();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = match answer {
                Some(a) => a.await.unwrap_or(Outcome::NotSent),
                None => Outcome::NotSent,
            };
            let _ = this.update_in(cx, |form, _, cx| {
                match &outcome {
                    Outcome::Accepted(v) => {
                        form.deletes_at = v["deletesAt"].as_i64();
                        form.sole_admin_of =
                            v["soleAdminOf"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect();
                        form.loaded = true;
                    }
                    other => form.error = form.app.read_with(cx, |app, _| app.call_failed(other)).ok(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn call(&mut self, mutation: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
        send(&self.app, mutation, json!({}), window, cx, |this, outcome, window, cx| {
            this.busy = false;
            if !matches!(outcome, Outcome::Accepted(_)) {
                this.error = this.app.read_with(cx, |app, _| app.call_failed(&outcome)).ok();
            }
            this.load(window, cx);
            cx.notify();
        });
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loaded && !self.busy && self.deletes_at.is_none() && self.sole_admin_of.is_empty() {
            self.call("account:requestDeletion", window, cx);
        }
    }
}

impl Render for DeleteAccountForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let body = v_flex().gap(px(space::L)).children(error_banner(&p, &self.error));
        if !self.loaded {
            return body.when(self.error.is_none(), |d| d.child(div().text_color(p.muted).child(t("Loading…", "Cargando…"))));
        }
        if let Some(ms) = self.deletes_at {
            let when = chrono::DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(&chrono::Local).date_naive());
            let ds = self.app.read_with(cx, |app, _| app.settings().date_style).ok();
            let when = when.zip(ds).map(|(d, s)| s.format(d)).unwrap_or_default();
            return body
                .child(div().typeset(text::BODY).child(tr!("Your account will be deleted on {when}.", "Tu cuenta se borrará el {when}.")))
                .child(footer(
                    &p,
                    t("Keep my account", "Conservar mi cuenta"),
                    true,
                    self.busy,
                    cx.listener(|this, _, w, cx| this.call("account:cancelDeletion", w, cx)),
                ));
        }
        if !self.sole_admin_of.is_empty() {
            let names = self.sole_admin_of.join(", ");
            return body.child(div().typeset(text::BODY).child(tr!(
                "You're the only admin of {names}. Make someone else an admin or delete the business before deleting your account.",
                "Eres el único administrador de {names}. Haz administrador a otra persona o borra el negocio antes de borrar tu cuenta."
            )));
        }
        body.child(div().typeset(text::BODY).text_color(p.muted).child(t(
            "After 7 days your sign-in and personal details are deleted and you leave every business. The records you made stay with each business, shown as “Former team member”. Encrypted backups expire within 90 days. Until then, you can cancel.",
            "Después de 7 días se borran tu acceso y tus datos personales y sales de todos los negocios. Los registros que hiciste quedan en cada negocio, como “Antiguo miembro del equipo”. Los respaldos cifrados vencen en 90 días. Hasta entonces, puedes cancelar.",
        )))
        .child(danger_footer("delete-account", t("Delete my account", "Borrar mi cuenta"), !self.busy, cx.listener(|this, _, w, cx| this.submit(w, cx))))
    }
}

pub struct DeleteBusinessForm {
    app: WeakEntity<VzApp>,
    business_id: String,
    name: String,
    pub confirm: Entity<InputState>,
    busy: bool,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

impl DeleteBusinessForm {
    pub fn new(app: WeakEntity<VzApp>, business_id: String, name: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let confirm = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![changed(&confirm, window, cx)];
        Self { app, business_id, name, confirm, busy: false, error: None, _subs: subs }
    }

    fn confirmed(&self, cx: &App) -> bool {
        value(&self.confirm, cx).trim() == self.name
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.confirmed(cx) || self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let args = json!({ "businessId": self.business_id, "name": self.name });
        send(&self.app, "businesses:remove", args, window, cx, |this, outcome, window, cx| {
            this.busy = false;
            if !matches!(outcome, Outcome::Accepted(_)) {
                this.error = this.app.read_with(cx, |app, _| app.call_failed(&outcome)).ok();
                return cx.notify();
            }
            window.close_dialog(cx);
            let id = this.business_id.clone();
            let _ = this.app.update(cx, |app, cx| app.business_deleted(&id, window, cx));
        });
    }
}

impl Render for DeleteBusinessForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let name = &self.name;
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.error))
            .child(div().typeset(text::BODY).text_color(p.muted).child(tr!(
                "All of {name}'s clients, loans, payments, money records, photos and members are deleted for everyone, on every device. This can't be undone. The books kept on this PC itself aren't touched.",
                "Se eliminan para todos y en todos los dispositivos los clientes, préstamos, pagos, movimientos de dinero, fotos y miembros de {name}. No se puede deshacer. Los registros guardados en esta PC no se tocan."
            )))
            .child(field(&p, &tr!("Type “{name}” to confirm", "Escribe “{name}” para confirmar"), Input::new(&self.confirm), None, None))
            .child(danger_footer(
                "delete-business",
                if self.busy { t("Deleting…", "Eliminando…") } else { t("Delete for everyone", "Eliminar para todos") },
                self.confirmed(cx) && !self.busy,
                cx.listener(|this, _, w, cx| this.submit(w, cx)),
            ))
    }
}

#[cfg(test)]
mod tests {
    // The kit's glob import also brings GPUI's `test` macro; keep the
    // built-in one for what `#[gpui::test]` expands to.
    use ::gpui;
    use core::prelude::v1::test;

    use super::*;
    use crate::services::Store;
    use crate::ui::forms::test_app;

    #[gpui::test]
    fn delete_waits_for_the_exact_name(cx: &mut TestAppContext) {
        let (app, cx) = test_app(cx, Store::in_memory());
        let form = cx.update(|window, cx| {
            let weak = app.downgrade();
            cx.new(|cx| DeleteBusinessForm::new(weak, "b1".into(), "VZ Capital".into(), window, cx))
        });
        for (typed, ok) in [("", false), ("vz capital", false), ("VZ Capita", false), (" VZ Capital ", true)] {
            cx.update(|window, cx| form.update(cx, |f, cx| f.confirm.update(cx, |s, cx| s.set_value(typed.to_string(), window, cx))));
            assert_eq!(cx.update(|_, cx| form.read(cx).confirmed(cx)), ok, "{typed:?}");
        }
        // Not signed in: nothing is sent, and the dialog says so and can be tried again.
        cx.update(|window, cx| form.update(cx, |f, cx| f.submit(window, cx)));
        cx.run_until_parked();
        assert!(!cx.update(|_, cx| form.read(cx).busy));
        assert!(cx.update(|_, cx| form.read(cx).error.is_some()));
    }

    #[gpui::test]
    fn a_new_business_needs_a_name(cx: &mut TestAppContext) {
        let (app, cx) = test_app(cx, Store::in_memory());
        let form = cx.update(|window, cx| {
            let weak = app.downgrade();
            cx.new(|cx| NewBusinessForm::new(weak, window, cx))
        });
        cx.update(|window, cx| form.update(cx, |f, cx| f.name.update(cx, |s, cx| s.set_value("   ".to_string(), window, cx))));
        assert_eq!(cx.update(|_, cx| form.read(cx).typed(cx)), "");
        cx.update(|window, cx| form.update(cx, |f, cx| f.name.update(cx, |s, cx| s.set_value(" Nuevo ".to_string(), window, cx))));
        assert_eq!(cx.update(|_, cx| form.read(cx).typed(cx)), "Nuevo");
    }
}
