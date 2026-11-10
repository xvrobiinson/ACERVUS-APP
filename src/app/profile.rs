//! The person's profile (roadmap 41, 2026-11-10): the name and photo the
//! account panel shows, read from the account (`account:profile`) once the
//! cloud can answer, and the window to change them (`ui::forms::profile`).

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
}

/// Two letters for a person without a photo: from the name, else the email.
pub(crate) fn initials_of(name: &str, email: &str) -> String {
    let source = if name.trim().is_empty() { email } else { name };
    source.split(['@', '.', ' ']).filter(|w| !w.is_empty()).take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect()
}

/// The photo's bytes, at most 4 MB.
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let mut res = crate::sync::auth::agent().get(url).call().map_err(|e| e.to_string())?;
    if res.status() != 200 {
        return Err(format!("HTTP {}", res.status()));
    }
    res.body_mut().with_config().limit(4 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())
}

impl VzApp {
    /// The signed-in person's name and email, as the panel shows them.
    pub(crate) fn who(&self) -> (String, String) {
        let email = self.cloud.as_ref().map(|c| c.email.clone()).unwrap_or_default();
        let name = if self.profile.email.eq_ignore_ascii_case(&email) { self.profile.name.clone() } else { String::new() };
        (name, email)
    }

    /// The photo, when the signed-in person has one.
    pub(crate) fn own_photo(&self) -> Option<Arc<Image>> {
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
                .update(cx, |app, cx| {
                    let email = v.get("email").and_then(Value::as_str).unwrap_or_default().to_string();
                    let same = app.profile.email.eq_ignore_ascii_case(&email) && app.profile.photo_url == url;
                    let photo = if same { app.profile.photo.clone() } else { None };
                    let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    app.profile = super::profile::Profile { email, name, photo_url: url.clone(), photo };
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

    /// Edit profile: the name and the photo.
    pub(crate) fn open_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (name, _) = self.who();
        let form = cx.new(|cx| crate::ui::forms::profile::ProfileForm::new(app, name, window, cx));
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
