//! The one-time offer to move the books kept on this PC into a new cloud
//! business (the app always shows a cloud business now). The cloud checks
//! every figure before it keeps anything, and the PC's own file is never
//! changed. Cancel asks again at the next sign-in; "Don't ask again"
//! never does.

use std::path::PathBuf;

use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::*;

use super::{error_banner, footer_with};
use crate::app::VzApp;
use crate::i18n::t;
use crate::sync::import::{self, Prepared};
use crate::sync::outbox::Outcome;
use crate::tr;
use crate::ui::kit::Button;
use crate::ui::theme::{Typeset as _, palette, space, text};

pub struct MoveBooksForm {
    app: WeakEntity<VzApp>,
    source: PathBuf,
    counts: import::Counts,
    busy: bool,
    error: Option<String>,
}

impl MoveBooksForm {
    pub fn new(app: WeakEntity<VzApp>, source: PathBuf) -> Self {
        let counts = import::counts(&source);
        Self { app, source, counts, busy: false, error: None }
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Ok(timezone) = self.app.read_with(cx, |app, _| app.new_business_timezone()) else { return };
        self.busy = true;
        self.error = None;
        cx.notify();
        let source = self.source.clone();
        let today = crate::utils::dates::today();
        let prepared = cx.background_spawn(async move { import::prepare(&source, &timezone, today) });
        cx.spawn_in(window, async move |this, cx| {
            let prepared = match prepared.await {
                Ok(p) => p,
                Err(e) => {
                    log::error!("books not prepared for the cloud: {e:#}");
                    let _ = this.update(cx, |form, cx| {
                        form.busy = false;
                        form.error = Some(tr!("These books couldn't be read: {e}", "No se pudieron leer estos registros: {e}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let answer = this
                .update(cx, |form, cx| form.app.read_with(cx, |app, _| app.cloud_call("imports:desktopBook", prepared.args.clone())))
                .ok()
                .and_then(Result::ok)
                .flatten();
            let outcome = match answer {
                Some(a) => a.await.unwrap_or(Outcome::NoAnswer),
                None => Outcome::NotSent,
            };
            let _ = this.update_in(cx, |form, window, cx| form.answered(outcome, prepared, window, cx));
        })
        .detach();
    }

    fn answered(&mut self, outcome: Outcome, prepared: Prepared, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        let Outcome::Accepted(done) = &outcome else {
            self.error = Some(match &outcome {
                Outcome::Refused(e) if e["code"] == "import_mismatch" => {
                    log::error!("cloud import refused, figures differ: {}", e["differences"]);
                    t(
                        "The cloud worked out some figures differently from this PC, so nothing was moved. Your books here are unchanged.",
                        "La nube calculó algunas cifras distinto que esta PC, así que no se movió nada. Tus registros aquí siguen igual.",
                    )
                    .to_string()
                }
                _ => self.app.read_with(cx, |app, _| app.call_failed(&outcome)).unwrap_or_default(),
            });
            return cx.notify();
        };
        let business = done["businessId"].as_str().unwrap_or_default().to_string();
        let client_ids = done["clientIds"].clone();
        window.close_dialog(cx);
        let _ = self.app.update(cx, |app, cx| {
            app.books_moved(&business, &client_ids, prepared, window, cx);
        });
    }

    /// Never offer again on this PC.
    fn never(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        let _ = self.app.update(cx, |app, _| app.never_move_books());
    }
}

impl Render for MoveBooksForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let import::Counts { clients, loans, payments } = self.counts;
        let never = Button::new("move-never")
            .ghost()
            .label(t("Don't ask again", "No volver a preguntar"))
            .on_click(cx.listener(|f, _, w, cx| f.never(w, cx)));
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.error))
            .child(div().typeset(text::BODY).text_color(p.text).child(tr!(
                "This PC has books that aren't in the cloud: {clients} clients, {loans} loans and {payments} payments.",
                "Esta PC tiene registros que no están en la nube: {clients} clientes, {loans} préstamos y {payments} pagos."
            )))
            .child(div().typeset(text::BODY).text_color(p.muted).child(t(
                "Moving them makes a new business you can open here and in the phone app. The cloud checks every figure first; if anything differs, nothing is moved. The file on this PC is never changed. If you already moved these books before, choose \"Don't ask again\".",
                "Moverlos crea un negocio nuevo que puedes abrir aquí y en la app del teléfono. La nube revisa cada cifra primero; si algo no cuadra, no se mueve nada. El archivo de esta PC nunca se cambia. Si ya moviste estos registros antes, elige \"No volver a preguntar\".",
            )))
            .child(footer_with(
                &p,
                never,
                if self.busy { t("Moving…", "Moviendo…") } else { t("Move to the cloud", "Mover a la nube") },
                !self.busy,
                self.busy,
                cx.listener(|f, _, w, cx| f.submit(w, cx)),
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
    use crate::domain::portfolio::CapitalKind;
    use crate::services::Store;
    use crate::ui::forms::test_app;
    use crate::utils::dates::today;

    /// Not signed in: nothing is sent, the dialog says why and can be tried again.
    #[gpui::test]
    fn moving_without_the_cloud_says_why(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("books.db");
        let mut s = Store::open(path.clone()).unwrap();
        s.add_capital(CapitalKind::Contribution, 500_000, today(), "", today()).unwrap();
        drop(s);
        let (app, cx) = test_app(cx, Store::in_memory());
        let form = cx.update(|_, cx| {
            let weak = app.downgrade();
            cx.new(|_| MoveBooksForm::new(weak, path.clone()))
        });
        assert_eq!(cx.update(|_, cx| form.read(cx).counts), import::Counts { clients: 0, loans: 0, payments: 0 });
        cx.update(|window, cx| form.update(cx, |f, cx| f.submit(window, cx)));
        cx.run_until_parked();
        assert!(!cx.update(|_, cx| form.read(cx).busy));
        assert!(cx.update(|_, cx| form.read(cx).error.is_some()));
    }
}
