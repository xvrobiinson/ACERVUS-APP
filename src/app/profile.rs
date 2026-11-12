//! The person's profile (roadmap 41, 2026-11-10): the name and photo the
//! account panel shows, read from the account (`account:profile`) once the
//! cloud can answer, and the window to change them (`ui::forms::profile`).
//! The same answer says when the account is waiting to be deleted (30 days,
//! S22 of 2026-11-12): the app then says so once with "Keep my account".

use std::cell::Cell;
use std::sync::Arc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::*;
use serde_json::{Value, json};

use super::VzApp;
use crate::i18n::t;
use crate::sync::outbox::Outcome;

/// What the account panel shows for the signed-in person.
#[derive(Clone, Default)]
pub(crate) struct Profile {
    /// Whose it is: shown only while that email is signed in.
    pub email: String,
    /// The name they gave ("" when none; never their email).
    pub name: String,
    pub photo_url: Option<String>,
    pub photo: Option<Arc<Image>>,
    /// The phone they give their teams ("" when none; the Team section).
    pub phone: String,
    /// Their teams don't see when they are online.
    pub hide_online: bool,
    /// When the account will be deleted, if they asked (ms since 1970).
    pub deletes_at: Option<i64>,
}

impl VzApp {
    /// "Your account will be deleted on …" with Keep my account (A14: a
    /// notice in both apps when a person waiting for deletion signs in).
    fn tell_deletion(&self, ms: i64, window: &mut Window, cx: &mut Context<Self>) {
        let style = self.settings().date_style;
        let when = chrono::DateTime::from_timestamp_millis(ms)
            .map(|d| style.format(d.with_timezone(&chrono::Local).date_naive()))
            .unwrap_or_default();
        let msg = crate::tr!(
            "Your account will be deleted on {when}. You can keep it, or download your data in the web app, until then.",
            "Tu cuenta se borrará el {when}. Hasta entonces puedes conservarla, o descargar tus datos en la app web."
        );
        let me = cx.entity().downgrade();
        let keep = move |window: &mut Window, cx: &mut App| {
            let _ = me.update(cx, |app, cx| app.keep_account(window, cx));
        };
        self.notify_with_action(msg, t("Keep my account", "Conservar mi cuenta"), keep, window, cx);
    }

    /// Cancels the account's deletion.
    fn keep_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(answer) = self.cloud_call("account:cancelDeletion", json!({})) else { return };
        cx.spawn_in(window, async move |this, cx| {
            let outcome = answer.await.unwrap_or(Outcome::NoAnswer);
            let _ = this.update_in(cx, |app, window, cx| match &outcome {
                Outcome::Accepted(_) => {
                    app.profile.deletes_at = None;
                    app.notify_success(t("Your account will be kept.", "Tu cuenta se conservará."), window, cx);
                }
                other => {
                    let msg = app.call_failed(other);
                    app.notify_error(msg, window, cx);
                }
            });
        })
        .detach();
    }
}

/// Two letters for a person without a photo: from the name, else the email.
pub(crate) fn initials_of(name: &str, email: &str) -> String {
    let source = if name.trim().is_empty() { email } else { name };
    source.split(['@', '.', ' ']).filter(|w| !w.is_empty()).take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect()
}

/// The photo's bytes, at most 4 MB.
pub(super) fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let mut res = crate::sync::auth::agent().get(url).call().map_err(|e| e.to_string())?;
    if res.status() != 200 {
        return Err(format!("HTTP {}", res.status()));
    }
    res.body_mut().with_config().limit(4 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())
}

impl VzApp {
    /// The signed-in person's name and email, as the panel shows them.
    pub(crate) fn who(&self) -> (String, String) {
        // Debug screenshots of the Team section show a made-up person.
        if self.crew.demo.is_some() {
            return ("Demo".into(), String::new());
        }
        let email = self.cloud.as_ref().map(|c| c.email.clone()).unwrap_or_default();
        let name = if self.profile.email.eq_ignore_ascii_case(&email) { self.profile.name.clone() } else { String::new() };
        (name, email)
    }

    /// The photo, when the signed-in person has one.
    pub(crate) fn own_photo(&self) -> Option<Arc<Image>> {
        if self.crew.demo.is_some() {
            return None;
        }
        let email = self.cloud.as_ref().map(|c| c.email.as_str()).unwrap_or_default();
        self.profile.photo.clone().filter(|_| self.profile.email.eq_ignore_ascii_case(email))
    }

    /// Reads the name and photo kept with the account.
    pub(crate) fn load_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(answer) = self.cloud_query("account:profile", json!({})) else { return };
        cx.spawn_in(window, async move |this, cx| {
            let Outcome::Accepted(v) = answer.await.unwrap_or(Outcome::NoAnswer) else { return };
            let url = v.get("photoUrl").and_then(Value::as_str).map(str::to_string);
            let fetch_url = this
                .update_in(cx, |app, window, cx| {
                    let email = v.get("email").and_then(Value::as_str).unwrap_or_default().to_string();
                    let same = app.profile.email.eq_ignore_ascii_case(&email) && app.profile.photo_url == url;
                    let photo = if same { app.profile.photo.clone() } else { None };
                    let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    let phone = v.get("phone").and_then(Value::as_str).unwrap_or_default().to_string();
                    let hide_online = v.get("hideOnline").and_then(Value::as_bool).unwrap_or(false);
                    let deletes_at = v.get("deletesAt").and_then(Value::as_f64).map(|ms| ms as i64);
                    // Said once when the app first learns of it, not on every reconnect.
                    let same_person = app.profile.email.eq_ignore_ascii_case(&email);
                    let tell = deletes_at.filter(|_| app.profile.deletes_at.is_none() || !same_person);
                    app.profile = super::profile::Profile { email, name, photo_url: url.clone(), photo, phone, hide_online, deletes_at };
                    if let Some(ms) = tell {
                        app.tell_deletion(ms, window, cx);
                    }
                    cx.notify();
                    url.filter(|_| app.profile.photo.is_none())
                })
                .ok()
                .flatten();
            let Some(url) = fetch_url else { return };
            let wanted = url.clone();
            let bytes = cx.background_spawn(async move { fetch(&url) }).await;
            match bytes {
                Ok(bytes) => {
                    let _ = this.update(cx, |app, cx| {
                        // Only if it is still the photo the account has.
                        if app.profile.photo_url.as_deref() == Some(wanted.as_str()) {
                            app.profile.photo = Some(Arc::new(Image::from_bytes(ImageFormat::Jpeg, bytes)));
                            cx.notify();
                        }
                    });
                }
                Err(e) => log::info!("profile photo not loaded: {e}"),
            }
        })
        .detach();
    }

    /// Edit profile: the name, the photo, the phone and whether the team
    /// sees when they are online.
    pub(crate) fn open_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (name, _) = self.who();
        let phone = self.profile.phone.clone();
        let form = cx.new(|cx| crate::ui::forms::profile::ProfileForm::new(app, name, phone, window, cx));
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            let dialog = dialog.entrance(DialogEntrance::FadeSlide);
            let handle = form.read(cx).name.focus_handle(cx);
            crate::ui::forms::focus_once(&first, &handle, window, cx);
            dialog.title(t("Your profile", "Tu perfil")).w(px(480.)).child(form.clone())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::initials_of;

    #[test]
    fn initials_come_from_the_name_else_the_email() {
        assert_eq!(initials_of("Marta Quiroga", "m@x.com"), "MQ");
        assert_eq!(initials_of("  ", "robin.valdez@example.com"), "RV");
    }
}
