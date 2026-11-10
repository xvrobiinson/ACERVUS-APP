//! Cloud sync on screen: the status chip and the "Cloud sync" card in
//! Settings (sign in with an emailed code, choose the business this PC
//! shows, review changes the cloud refused). The work itself is in
//! `app::cloud`.

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{SyncStatus, VzApp};
use crate::i18n::t;
use crate::sync::wire::Role;
use crate::tr;
use crate::ui::kit::{Button, ListRow};
use crate::ui::screens::icon;
use crate::ui::theme::{Palette, Typeset as _, space, text};

pub(crate) fn role_label(r: Role) -> &'static str {
    match r {
        Role::Admin => t("Admin", "Administrador"),
        Role::Collector => t("Collector", "Cobrador"),
        Role::Viewer => t("Viewer", "Solo lectura"),
    }
}

/// What a queued change was, for the review list.
fn change_label(mutation: &str) -> &'static str {
    match mutation {
        "clients:create" => t("New client", "Cliente nuevo"),
        "clients:update" => t("Client edit", "Cambio de cliente"),
        "clients:setArchived" => t("Client archived or restored", "Cliente archivado o restaurado"),
        "clients:remove" => t("Client deleted", "Cliente eliminado"),
        "loans:create" => t("New loan", "Préstamo nuevo"),
        "loans:update" => t("Loan edit", "Cambio de préstamo"),
        "loans:remove" => t("Loan deleted", "Préstamo eliminado"),
        "loans:markDefaulted" => t("Loan marked as defaulted", "Préstamo marcado incobrable"),
        "loans:reinstate" => t("Loan reinstated", "Préstamo reactivado"),
        "loans:setLateFeeWaived" => t("Late fee waived or restored", "Cargo por mora perdonado o restaurado"),
        "payments:record" => t("New payment", "Pago nuevo"),
        "payments:update" => t("Payment edit", "Cambio de pago"),
        "payments:remove" => t("Payment deleted", "Pago eliminado"),
        "money:addCapital" => t("Capital added or withdrawn", "Capital agregado o retirado"),
        "money:updateCapital" => t("Capital edit", "Cambio de capital"),
        "money:removeCapital" => t("Capital deleted", "Capital eliminado"),
        "money:addEntry" => t("New expense, income or transfer", "Gasto, ingreso o transferencia nueva"),
        "money:adjustBalances" => t("Balance adjustment", "Ajuste de saldos"),
        "money:updateEntry" => t("Money record edit", "Cambio de movimiento"),
        "money:removeEntry" => t("Money record deleted", "Movimiento eliminado"),
        "settings:update" => t("Settings", "Configuración"),
        "settings:setLogo" => t("Logo", "Logo"),
        _ => t("Change", "Cambio"),
    }
}

impl VzApp {
    /// The sync status as icon, words and whether it needs the person.
    /// None when not signed in.
    pub(crate) fn sync_look(&self) -> Option<(IconName, String, bool)> {
        Some(match self.sync_status()? {
            SyncStatus::NotLinked => (IconName::Cloud, t("Choose a business", "Elige un negocio").to_string(), true),
            SyncStatus::Synced => (IconName::CloudCheck, t("Synced", "Sincronizado").to_string(), false),
            SyncStatus::Sending(n) => (IconName::CloudUpload, tr!("Sending {n}", "Enviando {n}"), false),
            SyncStatus::Offline(0) => (IconName::CloudOff, t("Offline", "Sin conexión").to_string(), false),
            SyncStatus::Offline(n) => (IconName::CloudOff, tr!("Offline · {n} waiting", "Sin conexión · {n} en espera"), false),
            SyncStatus::NeedsReview(n) => (IconName::CloudAlert, tr!("{n} to review", "{n} por revisar"), true),
            SyncStatus::UpdateRequired => (IconName::CloudAlert, t("Update needed", "Actualización necesaria").to_string(), true),
        })
    }

    /// The body of the "Cloud sync" page in Settings (option B, 2026-11-10,
    /// the owner's pick): a band that says how sync stands and who is signed
    /// in, the changes to review, then Businesses and Google Drive backups as
    /// two small cards side by side, and "Delete my account" at the foot.
    pub(crate) fn sync_settings(&mut self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(cloud) = &self.cloud else { return self.sign_in_form(p, cx) };
        let email = cloud.email.clone();
        let (glyph, words, _) = self.sync_look().unwrap_or((IconName::Cloud, String::new(), false));
        let status = self.sync_status();
        let (ink, tint) = match status {
            Some(SyncStatus::Synced) => (p.positive, p.positive_tint),
            Some(SyncStatus::NeedsReview(_)) | Some(SyncStatus::UpdateRequired) => (p.negative, p.negative_tint),
            _ => (p.warning, p.warning_tint),
        };
        let band = h_flex()
            .flex_wrap()
            .gap(px(space::M))
            .px(px(space::M))
            .py(px(10.))
            .rounded(px(12.))
            .bg(tint)
            .child(icon(glyph, ink, 16.))
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(180.))
                    .gap(px(6.))
                    .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(words))
                    .child(div().text_color(p.muted).child("·"))
                    .child(div().min_w_0().truncate().text_color(p.muted).child(email)),
            )
            .child(
                Button::new("sync-sign-out")
                    .ghost()
                    .small()
                    .label(t("Sign out", "Cerrar sesión"))
                    .on_click(cx.listener(|this, _, w, cx| this.sign_out(w, cx))),
            );
        // What the band's words don't say: what to do about it.
        let detail = match status {
            Some(SyncStatus::Offline(0)) => Some(t("You can keep working; new clients, payments, expenses and income are sent when the connection is back.", "Puedes seguir trabajando; los clientes nuevos, pagos, gastos e ingresos se envían cuando vuelva la conexión.").to_string()),
            Some(SyncStatus::Offline(n)) => Some(tr!("{n} changes wait and are sent when the connection is back.", "{n} cambios esperan y se envían cuando vuelva la conexión.")),
            Some(SyncStatus::NeedsReview(n)) => Some(tr!("{n} changes weren't accepted by the cloud. Review them below.", "La nube no aceptó {n} cambios. Revísalos abajo.")),
            Some(SyncStatus::NotLinked) => Some(t(
                "Choose the business to show on this PC. Its books come from the cloud and stay in step with the phone app.",
                "Elige el negocio que se verá en esta PC. Sus registros vienen de la nube y se mantienen al día con la app del teléfono.",
            ).to_string()),
            _ => None,
        };
        let delete_account = h_flex().justify_end().child(
            Button::new("sync-delete-account")
                .danger_ghost()
                .small()
                .label(t("Delete my account…", "Borrar mi cuenta…"))
                .on_click(cx.listener(|this, _, w, cx| this.open_delete_account(w, cx))),
        );
        let body =
            v_flex().gap(px(space::M)).child(band).children(detail.map(|d| div().typeset(text::CAPTION).text_color(p.muted).child(d)));

        if let Some(min) = &cloud.update_required {
            return body
                .child(div().text_color(p.text).child(tr!(
                    "This version of the app can't sync anymore. Install version {min} or newer; nothing is sent until then.",
                    "Esta versión de la app ya no puede sincronizar. Instala la versión {min} o una más nueva; no se envía nada hasta entonces."
                )))
                .child(delete_account)
                .into_any_element();
        }

        let refused = self.store.outbox_refused().unwrap_or_default();
        let review: Vec<AnyElement> = refused
            .into_iter()
            .map(|r| {
                let why = crate::sync::refusal::describe(&r.error, self.settings());
                let (retry, discard) = (r.op_id.clone(), r.op_id.clone());
                ListRow::new(SharedString::from(format!("sync-refused-{}", r.op_id)))
                    .title(change_label(r.op.mutation))
                    .subtitle(why)
                    .trailing(
                        Button::new(SharedString::from(format!("sync-retry-{}", r.op_id)))
                            .label(t("Send again", "Enviar de nuevo"))
                            .on_click(cx.listener(move |this, _, w, cx| this.retry_refused(&retry, w, cx))),
                    )
                    .trailing(
                        Button::new(SharedString::from(format!("sync-discard-{}", r.op_id)))
                            .danger_ghost()
                            .label(t("Discard", "Descartar"))
                            .on_click(cx.listener(move |this, _, w, cx| this.discard_refused(&discard, w, cx))),
                    )
                    .into_any_element()
            })
            .collect();
        let choosing = cloud.linked.is_none();
        let cards = h_flex()
            .flex_wrap()
            .items_start()
            .gap(px(space::M))
            .when(choosing, |d| d.child(self.business_list(p, cx)))
            .child(self.drive_settings(p, cx));
        body.children((!review.is_empty()).then(|| v_flex().mx(px(-space::XL)).children(review)))
            .child(cards)
            .child(delete_account)
            .into_any_element()
    }

    /// One of the page's two small cards.
    fn sync_card(p: &Palette) -> Div {
        v_flex().flex_1().min_w(px(240.)).gap(px(space::S)).p(px(space::M)).rounded(px(12.)).border_1().border_color(p.border)
    }

    /// Every business the person belongs to, one short line each: the one
    /// this PC shows is marked; others open with Open; admins can delete.
    fn business_list(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(cloud) = &self.cloud else { return div().into_any_element() };
        let online = cloud.online;
        let note = |s: &'static str| div().typeset(text::CAPTION).text_color(p.muted).child(s).into_any_element();
        let rows: Vec<AnyElement> = match &cloud.businesses {
            None => vec![note(t("Getting your businesses…", "Buscando tus negocios…"))],
            Some(list) if list.is_empty() => vec![note(t(
                "You don't belong to a business yet. Create one, or ask an admin to invite you.",
                "Aún no perteneces a un negocio. Crea uno o pide a un administrador que te invite.",
            ))],
            Some(list) => list
                .iter()
                .map(|b| {
                    let (id, name) = (b.business_id.clone(), b.name.clone());
                    let shown = cloud.linked.as_deref() == Some(id.as_str());
                    let open_id = id.clone();
                    h_flex()
                        .gap(px(space::S))
                        .min_h(px(30.))
                        .child(div().flex_none().size(px(8.)).rounded_full().bg(if shown { p.positive } else { p.border }))
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(p.text)
                                        .when(shown, |d| d.font_weight(FontWeight::SEMIBOLD))
                                        .child(name.clone()),
                                )
                                .child(div().flex_none().typeset(text::CAPTION).text_color(p.faint).child(role_label(b.role))),
                        )
                        .when(!shown, |r| {
                            r.child(
                                Button::new(SharedString::from(format!("sync-open-{id}")))
                                    .small()
                                    .label(t("Open", "Abrir"))
                                    .on_click(cx.listener(move |this, _, w, cx| this.link_business(&open_id, w, cx))),
                            )
                        })
                        .when(b.role == Role::Admin, |r| {
                            let (id, name) = (id.clone(), name.clone());
                            r.child(
                                Button::new(SharedString::from(format!("sync-delete-{id}")))
                                    .danger_ghost()
                                    .small()
                                    .icon(IconName::Trash)
                                    .tooltip(t("Delete business", "Eliminar negocio"))
                                    .disabled(!online)
                                    .on_click(
                                        cx.listener(move |this, _, w, cx| this.open_delete_business(id.clone(), name.clone(), w, cx)),
                                    ),
                            )
                        })
                        .into_any_element()
                })
                .collect(),
        };
        Self::sync_card(p)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(t("Businesses", "Negocios")))
                    .child(
                        Button::new("sync-new-business")
                            .small()
                            .icon(IconName::Plus)
                            .label(t("New", "Nuevo"))
                            .disabled(!online)
                            .tooltip(t("New business. Needs a connection.", "Negocio nuevo. Necesita conexión."))
                            .on_click(cx.listener(|this, _, w, cx| this.open_new_business(w, cx))),
                    ),
            )
            .children(rows)
            .into_any_element()
    }

    /// Backups to Google Drive: off with a "Turn on" button, waiting for the
    /// browser, or on with the last upload and "Back up now". The long
    /// explanation shows only while Drive isn't connected.
    fn drive_settings(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = &self.cloud else { return div().into_any_element() };
        let d = &c.drive;
        let title = div().typeset(text::BODY_STRONG).text_color(p.text).child(t("Google Drive backups", "Copias en Google Drive"));
        let error = d.error.clone().map(|e| div().typeset(text::CAPTION).text_color(p.negative).child(e));
        let small = |s: String| div().typeset(text::CAPTION).text_color(p.muted).child(s);
        let card = Self::sync_card(p).child(title);
        let card = if d.linking.is_some() {
            card.child(small(
                t(
                    "Finish in your browser: choose your Google account and allow access to the files this app creates.",
                    "Termina en el navegador: elige tu cuenta de Google y permite el acceso a los archivos que crea esta app.",
                )
                .to_string(),
            ))
            .child(
                h_flex()
                    .gap(px(space::S))
                    .child(
                        Button::new("drive-waiting").small().label(t("Waiting for your browser…", "Esperando el navegador…")).loading(true),
                    )
                    .child(
                        Button::new("drive-cancel")
                            .ghost()
                            .small()
                            .label(t("Cancel", "Cancelar"))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_drive_link(cx))),
                    ),
            )
        } else if !self.drive_on() {
            let connected = d.connected == Some(true);
            let (words, label) = if connected {
                (
                    t(
                        "Off for these books. Turn it on to keep a copy in your Google Drive every day.",
                        "Desactivadas para estos registros. Actívalas para guardar una copia en tu Google Drive cada día.",
                    ),
                    t("Turn on", "Activar"),
                )
            } else {
                (
                    t(
                        "Keep a copy of these books in your own Google Drive, every day and whenever you back up. Connect the Google account that uses the same email as this account. The app can see only the files it saves there; the newest 30 are kept.",
                        "Guarda una copia de estos registros en tu propio Google Drive, cada día y cada vez que hagas una copia. Conecta la cuenta de Google que usa el mismo correo que esta cuenta. La app solo ve los archivos que guarda ahí; se guardan las 30 más recientes.",
                    ),
                    t("Connect Google Drive", "Conectar Google Drive"),
                )
            };
            card.child(small(words.to_string())).children(error).child(
                h_flex().child(
                    Button::new("drive-on")
                        .small()
                        .icon(IconName::CloudUpload)
                        .label(label)
                        .disabled(!c.online)
                        .on_click(cx.listener(|this, _, w, cx| this.turn_on_drive(w, cx))),
                ),
            )
        } else {
            let last = match self.drive_last() {
                Some(when) => {
                    let when = when.format("%Y-%m-%d %H:%M").to_string();
                    tr!("On · last {when}", "Activadas · última {when}")
                }
                None => t("On · no backup yet", "Activadas · aún sin copias").to_string(),
            };
            card.child(small(last)).children(error).child(
                h_flex()
                    .flex_wrap()
                    .gap(px(space::S))
                    .child(
                        Button::new("drive-now")
                            .small()
                            .icon(IconName::CloudUpload)
                            .label(t("Back up now", "Copiar ahora"))
                            .loading(d.uploading)
                            .on_click(cx.listener(|this, _, w, cx| this.drive_backup(false, w, cx))),
                    )
                    .child(
                        Button::new("drive-off")
                            .ghost()
                            .small()
                            .label(t("Turn off", "Desactivar"))
                            .tooltip(t("Backups already in Drive stay there.", "Las copias que ya están en Drive se quedan ahí."))
                            .on_click(cx.listener(|this, _, _, cx| this.turn_off_drive(cx))),
                    ),
            )
        };
        card.into_any_element()
    }

    /// Signed out: what signing in does, and the button that opens the
    /// sign-in screen.
    fn sign_in_form(&mut self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap(px(space::M))
            .child(div().text_color(p.text).child(t(
                "Sign in to see your business from the phone app on this PC and keep both in step. Use an emailed code or your Google account.",
                "Inicia sesión para ver en esta PC tu negocio de la app del teléfono y mantener ambos al día. Usa un código por correo o tu cuenta de Google.",
            )))
            .child(
                h_flex().child(
                    Button::new("sync-open-sign-in")
                        .primary()
                        .icon(IconName::LogIn)
                        .label(t("Sign in", "Iniciar sesión"))
                        .on_click(cx.listener(|this, _, w, cx| this.open_sign_in(w, cx))),
                ),
            )
            .child(div().typeset(text::CAPTION).text_color(p.muted).child(t(
                "Without signing in, the app works only with the books on this PC and makes no network requests.",
                "Sin iniciar sesión, la app trabaja solo con los registros de esta PC y no usa internet.",
            )))
            .into_any_element()
    }
}
