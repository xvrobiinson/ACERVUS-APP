//! The person's profile (roadmap 41, 2026-11-10; the web's
//! src/ui/profile.tsx): their name, shown in the account panel and to their
//! team, and a photo they can add, change or remove (initials without one).
//! Kept with the account (convex/account.ts), so every phone and PC shows
//! the same. A photo change is sent at once; the name with Save. Since the
//! Team section (2026-11-11) their teams see the photo too, the phone they
//! choose to give (sent with Save), and when they are online unless they
//! turn that off (sent at once).

use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use super::{error_banner, field, footer, value};
use crate::app::VzApp;
use crate::i18n::t;
use crate::services::picture;
use crate::sync::outbox::Outcome;
use crate::tr;
use crate::ui::kit::Button;
use crate::ui::theme::{Typeset as _, palette, space, text};

pub struct ProfileForm {
    app: WeakEntity<VzApp>,
    pub name: Entity<InputState>,
    saved_name: String,
    phone: Entity<InputState>,
    saved_phone: String,
    busy: bool,
    photo_busy: bool,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

impl ProfileForm {
    pub fn new(app: WeakEntity<VzApp>, name: String, phone: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name.clone()));
        let phone_input = cx.new(|cx| InputState::new(window, cx).default_value(phone.clone()));
        let changed = |_: &mut Self, _: &Entity<InputState>, ev: &InputEvent, _: &mut Window, cx: &mut Context<Self>| {
            if let InputEvent::Change = ev {
                cx.notify()
            }
        };
        let subs = vec![cx.subscribe_in(&input, window, changed), cx.subscribe_in(&phone_input, window, changed)];
        Self {
            app,
            name: input,
            saved_name: name,
            phone: phone_input,
            saved_phone: phone,
            busy: false,
            photo_busy: false,
            error: None,
            _subs: subs,
        }
    }

    fn typed_phone(&self, cx: &App) -> String {
        value(&self.phone, cx).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Whether the team sees when they are online: sent at once.
    fn set_show_online(&mut self, show: bool, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.app.update(cx, |app, cx| {
            app.profile.hide_online = !show;
            cx.notify();
        });
        cx.notify();
        self.send("account:setHideOnline", json!({ "hide": !show }), window, cx, move |this, outcome, _, cx| {
            if !matches!(outcome, Outcome::Accepted(_)) {
                let _ = this.app.update(cx, |app, cx| {
                    app.profile.hide_online = show;
                    cx.notify();
                });
                this.failed(&outcome, cx);
            }
        });
    }

    fn typed(&self, cx: &App) -> String {
        value(&self.name, cx).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Sends `mutation` and hands the answer back here.
    fn send(
        &self,
        mutation: &'static str,
        args: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Outcome, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let answer = self.app.read_with(cx, |app, _| app.cloud_call(mutation, args)).ok().flatten();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = match answer {
                Some(answer) => answer.await.unwrap_or(Outcome::NotSent),
                None => Outcome::NotSent,
            };
            let _ = this.update_in(cx, |form, window, cx| then(form, outcome, window, cx));
        })
        .detach();
    }

    fn failed(&mut self, outcome: &Outcome, cx: &mut Context<Self>) {
        self.error = self.app.read_with(cx, |app, _| app.call_failed(outcome)).ok();
        cx.notify();
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.typed(cx);
        if self.busy {
            return;
        }
        if name.is_empty() {
            self.error = Some(t("Write your name", "Escribe tu nombre").into());
            return cx.notify();
        }
        let phone = self.typed_phone(cx);
        if name == self.saved_name && phone == self.saved_phone {
            window.close_dialog(cx);
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        if name != self.saved_name {
            self.send("account:setName", json!({ "name": name }), window, cx, move |this, outcome, window, cx| {
                if !matches!(outcome, Outcome::Accepted(_)) {
                    this.busy = false;
                    return this.failed(&outcome, cx);
                }
                this.saved_name = name.clone();
                let _ = this.app.update(cx, |app, cx| {
                    app.profile.name = name.clone();
                    cx.notify();
                });
                this.save_phone(window, cx);
            });
        } else {
            self.save_phone(window, cx);
        }
    }

    /// The phone, when it changed; then the window closes.
    fn save_phone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let phone = self.typed_phone(cx);
        let done = |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            this.busy = false;
            window.close_dialog(cx);
            let _ = this.app.update(cx, |app, cx| app.notify_success(t("Profile saved", "Perfil guardado"), window, cx));
        };
        if phone == self.saved_phone {
            return done(self, window, cx);
        }
        self.send("account:setPhone", json!({ "phone": phone }), window, cx, move |this, outcome, window, cx| {
            if !matches!(outcome, Outcome::Accepted(_)) {
                this.busy = false;
                return this.failed(&outcome, cx);
            }
            this.saved_phone = phone.clone();
            let _ = this.app.update(cx, |app, cx| {
                app.profile.phone = phone.clone();
                cx.notify();
            });
            done(this, window, cx);
        });
    }

    fn choose_photo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let started = self.app.update(cx, |app, cx| app.begin_file_dialog("profile photo", window, cx)).unwrap_or(false);
        if !started {
            return;
        }
        let request = crate::file_picker::Request::picture(t("Choose your photo", "Elige tu foto"), t("Use this photo", "Usar esta foto"));
        let rx = crate::file_picker::pick(crate::file_picker::owner_of(window), request);
        cx.spawn_in(window, async move |this, cx| {
            let result = crate::file_picker::wait(rx, cx.background_executor().clone()).await;
            let _ = this.update_in(cx, |form, window, cx| {
                let picked = form.app.update(cx, |app, _| app.end_pick("profile photo", result));
                match picked {
                    Ok(Ok(Some(path))) => form.use_photo_file(path, window, cx),
                    Ok(Err(why)) => {
                        form.error = Some(tr!("The Open window couldn't open ({why}).", "La ventana Abrir no se pudo abrir ({why})."));
                        cx.notify();
                    }
                    _ => {}
                }
            });
        })
        .detach();
    }

    /// Reads and squares the picture away from the window, then sends it.
    fn use_photo_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.photo_busy = true;
        self.error = None;
        cx.notify();
        let task = cx.background_spawn(async move {
            let bytes = std::fs::read(&path).map_err(|e| {
                let why = e.kind().to_string();
                tr!("Couldn't open the file ({why}).", "No se pudo abrir el archivo ({why}).")
            })?;
            picture::profile_jpeg(&bytes).map_err(|e| e.message().to_string())
        });
        cx.spawn_in(window, async move |this, cx| {
            let jpeg = task.await;
            let _ = this.update_in(cx, |form, window, cx| match jpeg {
                Err(why) => {
                    form.photo_busy = false;
                    form.error = Some(why);
                    cx.notify();
                }
                Ok(jpeg) => {
                    use base64::Engine as _;
                    let upload = json!({ "contentType": "image/jpeg", "data": base64::engine::general_purpose::STANDARD.encode(&jpeg) });
                    form.send("account:setPhoto", json!({ "upload": upload }), window, cx, move |this, outcome, window, cx| {
                        this.photo_busy = false;
                        if !matches!(outcome, Outcome::Accepted(_)) {
                            return this.failed(&outcome, cx);
                        }
                        let _ = this.app.update(cx, |app, cx| {
                            app.profile.photo = Some(std::sync::Arc::new(Image::from_bytes(ImageFormat::Jpeg, jpeg)));
                            app.load_profile(window, cx);
                            cx.notify();
                        });
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn remove_photo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.photo_busy = true;
        self.error = None;
        cx.notify();
        self.send("account:removePhoto", json!({}), window, cx, |this, outcome, _, cx| {
            this.photo_busy = false;
            if !matches!(outcome, Outcome::Accepted(_)) {
                return this.failed(&outcome, cx);
            }
            let _ = this.app.update(cx, |app, cx| {
                app.profile.photo = None;
                app.profile.photo_url = None;
                cx.notify();
            });
            cx.notify();
        });
    }
}

impl Render for ProfileForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let (photo, email, hide_online) = self
            .app
            .read_with(cx, |app, _| {
                (app.profile.photo.clone(), app.cloud.as_ref().map(|c| c.email.clone()).unwrap_or_default(), app.profile.hide_online)
            })
            .unwrap_or_default();
        let typed = self.typed(cx);
        let phone = self.typed_phone(cx);
        let letters = crate::app::initials_of(&typed, &email);
        let face = h_flex()
            .flex_none()
            .size(px(72.))
            .rounded_full()
            .overflow_hidden()
            .justify_center()
            .bg(p.accent_tint)
            .text_color(p.accent_text)
            .typeset(text::TITLE)
            .child(match &photo {
                Some(image) => img(image.clone()).size(px(72.)).rounded_full().object_fit(ObjectFit::Cover).into_any_element(),
                None => div().child(letters).into_any_element(),
            });
        let has = photo.is_some();
        let buttons = h_flex()
            .gap(px(space::S))
            .child(
                Button::new("profile-photo-pick")
                    .small()
                    .icon(IconName::Upload)
                    .label(if has { t("Change photo…", "Cambiar foto…") } else { t("Add a photo…", "Agregar una foto…") })
                    .loading(self.photo_busy)
                    .on_click(cx.listener(|this, _, w, cx| this.choose_photo(w, cx))),
            )
            .when(has && !self.photo_busy, |d| {
                d.child(
                    Button::new("profile-photo-remove")
                        .small()
                        .danger_ghost()
                        .label(t("Remove", "Quitar"))
                        .on_click(cx.listener(|this, _, w, cx| this.remove_photo(w, cx))),
                )
            });
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.error))
            .child(h_flex().gap(px(space::L)).items_center().child(face).child(buttons))
            .child(field(
                &p,
                t("Your name", "Tu nombre"),
                Input::new(&self.name),
                Some(t("Your team sees it, and the greeting uses it.", "Tu equipo lo ve, y el saludo lo usa.").into()),
                None,
            ))
            .child(field(
                &p,
                t("Phone (optional)", "Teléfono (opcional)"),
                Input::new(&self.phone),
                Some(t("Your team can write to you on WhatsApp.", "Tu equipo puede escribirte por WhatsApp.").into()),
                None,
            ))
            .child(
                v_flex()
                    .gap(px(space::XS))
                    .child(
                        Checkbox::new("profile-show-online")
                            .label(t("Show my team when I'm online", "Mostrar a mi equipo cuando estoy en línea"))
                            .checked(!hide_online)
                            .on_click(cx.listener(|this, on: &bool, w, cx| this.set_show_online(*on, w, cx))),
                    )
                    .child(div().pl(px(24.)).typeset(text::CAPTION).text_color(p.muted).child(t(
                        "Off: nobody sees whether you're online or when you were last.",
                        "Apagado: nadie ve si estás en línea ni cuándo estuviste por última vez.",
                    ))),
            )
            .child(div().typeset(text::CAPTION).text_color(p.muted).child(tr!("Signed in as {email}", "Sesión iniciada como {email}")))
            .child(footer(
                &p,
                t("Save", "Guardar"),
                !typed.is_empty() && (typed != self.saved_name || phone != self.saved_phone),
                self.busy,
                cx.listener(|this, _, w, cx| this.submit(w, cx)),
            ))
    }
}
