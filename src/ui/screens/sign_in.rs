//! The sign-in screen (the web app's look, 2026-11-07): the hero's picture
//! (fog in light, streaks in dark), the brand in the corner and one solid
//! card in the panel's colors holding the email
//! code steps (signing in, or making an account with a name) and "Continue
//! with Google". It takes the whole window while open. The app needs an
//! account, so it opens at launch and after signing out with no way back;
//! only debug builds on a demo database show "Back to the app". The work is
//! in `app::cloud`.

use std::sync::{Arc, LazyLock};

use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::app::VzApp;
use crate::i18n::t;
use crate::tr;
use crate::ui::components::icon;
use crate::ui::kit::Button;
use crate::ui::theme::{Elevation, Palette, RADIUS_CONTROL, Typeset as _, elevation, palette, space, text};

const CARD_W: f32 = 380.0;

/// Google's "G" in its own colors, as Google's sign-in branding asks.
static GOOGLE_G: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Svg, include_bytes!("../../../assets/google-g.svg").to_vec())));

/// Under the picture while it loads: its own average tone.
fn h_base(p: &Palette) -> Hsla {
    p.splash.base
}

/// The square tile with the step's icon at the top of the card.
fn tile(p: &Palette, name: IconName) -> Div {
    div()
        .size(px(52.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(14.))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .shadow(elevation(p, Elevation::Raised))
        .child(icon(name, p.text, 22.))
}

impl VzApp {
    pub(crate) fn render_sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let size = window.viewport_size();
        let w = f32::from(size.width);
        let s = &self.sign_in;
        let (creating, required) = (s.creating, s.required);
        let error = s.error.clone().map(|e| div().text_color(p.negative).typeset(text::CAPTION).child(e));

        let (glyph, title, sub, form) = match s.code_sent_to.clone() {
            None => {
                let google = match &s.google {
                    None => Button::new("sign-in-google")
                        .prefix(img(GOOGLE_G.clone()).size(px(18.)))
                        .label(t("Continue with Google", "Continuar con Google"))
                        .wide()
                        .on_click(cx.listener(|this, _, w, cx| this.sign_in_with_google(w, cx)))
                        .into_any_element(),
                    Some(_) => v_flex()
                        .gap(px(space::S))
                        .items_center()
                        .child(
                            Button::new("sign-in-google-waiting")
                                .prefix(img(GOOGLE_G.clone()).size(px(18.)))
                                .label(t("Waiting for your browser…", "Esperando el navegador…"))
                                .loading(true)
                                .wide(),
                        )
                        .child(
                            Button::new("sign-in-google-cancel")
                                .ghost()
                                .small()
                                .label(t("Cancel", "Cancelar"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_google(cx))),
                        )
                        .into_any_element(),
                };
                // Signing in, or making a new account (which asks for a name).
                let (question, other, to_creating) = if creating {
                    (t("Already have an account?", "¿Ya tienes una cuenta?"), t("Sign in", "Iniciar sesión"), false)
                } else {
                    (t("New here?", "¿Eres nuevo?"), t("Create an account", "Crear una cuenta"), true)
                };
                let switch =
                    h_flex().justify_center().items_center().gap(px(2.)).typeset(text::CAPTION).text_color(p.muted).child(question).child(
                        Button::new("sign-in-switch")
                            .ghost()
                            .small()
                            .label(other)
                            .on_click(cx.listener(move |this, _, w, cx| this.set_creating(to_creating, w, cx))),
                    );
                let form = v_flex()
                    .gap(px(space::M))
                    .children(creating.then(|| Input::new(&s.name).large().prefix(icon(IconName::UserPlus, p.faint, 16.))))
                    .child(Input::new(&s.email).large().prefix(icon(IconName::Mail, p.faint, 16.)))
                    .children(error)
                    .child(
                        Button::new("sign-in-send-code")
                            .primary()
                            .label(t("Send code", "Enviar código"))
                            .loading(s.busy)
                            .wide()
                            .on_click(cx.listener(|this, _, w, cx| this.send_sign_in_code(w, cx))),
                    )
                    .child(
                        h_flex()
                            .gap(px(space::M))
                            .typeset(text::CAPTION)
                            .text_color(p.faint)
                            .child(div().flex_1().h(px(1.)).bg(p.border))
                            .child(t("Or continue with", "O continúa con"))
                            .child(div().flex_1().h(px(1.)).bg(p.border)),
                    )
                    .child(google)
                    .child(switch);
                if creating {
                    (
                        IconName::UserPlus,
                        t("Create an account", "Crear una cuenta").to_string(),
                        t("Then create your business or accept an invitation.", "Luego crea tu negocio o acepta una invitación.")
                            .to_string(),
                        form,
                    )
                } else {
                    (
                        IconName::LogIn,
                        t("Sign in to Acervus", "Inicia sesión en Acervus").to_string(),
                        t(
                            "We'll email you a 6-digit code. No password to remember.",
                            "Te enviaremos un código de 6 dígitos. Sin contraseñas que recordar.",
                        )
                        .to_string(),
                        form,
                    )
                }
            }
            Some(email) => {
                let form = v_flex()
                    .gap(px(space::M))
                    .child(Input::new(&s.code).large().prefix(icon(IconName::KeyRound, p.faint, 16.)))
                    .child(div().typeset(text::CAPTION).text_color(p.muted).child(t(
                        "It expires in 10 minutes. Check your spam folder too.",
                        "Vence en 10 minutos. Revisa también el correo no deseado.",
                    )))
                    .children(error)
                    .child(
                        Button::new("sign-in-finish")
                            .primary()
                            .label(t("Sign in", "Iniciar sesión"))
                            .loading(s.busy)
                            .wide()
                            .on_click(cx.listener(|this, _, w, cx| this.finish_sign_in(w, cx))),
                    )
                    .child(
                        h_flex().justify_center().child(
                            Button::new("sign-in-other-email")
                                .ghost()
                                .small()
                                .label(t("Use a different email", "Usar otro correo"))
                                .on_click(cx.listener(|this, _, _, cx| this.restart_sign_in(cx))),
                        ),
                    );
                (
                    IconName::KeyRound,
                    t("Enter your code", "Escribe tu código").to_string(),
                    tr!("We sent a 6-digit code to {email}.", "Enviamos un código de 6 dígitos a {email}."),
                    form,
                )
            }
        };

        let note = (!required).then(|| {
            div()
                .text_center()
                .typeset(text::CAPTION)
                .text_color(p.faint)
                .child(t(
                    "Without signing in, the app works only with the books on this PC.",
                    "Sin iniciar sesión, la app trabaja solo con los registros de esta PC.",
                ))
                .into_any_element()
        });
        // Only when the app can be used without an account.
        let back = (!required).then(|| {
            Button::new("sign-in-back")
                .ghost()
                .icon(IconName::ArrowLeft)
                .label(t("Back to the app", "Volver a la app"))
                .on_click(cx.listener(|this, _, w, cx| this.close_sign_in(w, cx)))
                .into_any_element()
        });
        glass_screen(&p, w, "sign-in", GlassCard { glyph, title, sub, body: form.into_any_element(), note }, back)
    }
}

/// The sign-in card: a tile with an icon, a title, a line under
/// it, the card's content and an optional faint note at the foot.
pub(crate) struct GlassCard {
    pub glyph: IconName,
    pub title: String,
    pub sub: String,
    pub body: AnyElement,
    pub note: Option<AnyElement>,
}

/// The sign-in look: the hero's picture with the brand in the corner, an
/// optional button in the other corner, and one solid card in the middle.
/// The business welcome uses it too.
pub(crate) fn glass_screen(p: &Palette, w: f32, id: &'static str, card: GlassCard, corner: Option<AnyElement>) -> AnyElement {
    let GlassCard { glyph, title, sub, body, note } = card;
    let h = p.hero();
    let card = v_flex()
        .w(px(CARD_W.min(w - 2. * space::L)))
        .px(px(28.))
        .pt(px(32.))
        .pb(px(28.))
        .gap(px(space::L))
        .rounded(px(22.))
        .border_1()
        .border_color(p.border)
        .bg(p.surface)
        .shadow(elevation(p, Elevation::Floating))
        .child(
            v_flex()
                .items_center()
                .gap(px(6.))
                .child(tile(p, glyph))
                .child(div().mt(px(10.)).typeset(text::TITLE).text_color(p.text).child(title))
                .child(div().text_center().text_color(p.muted).max_w(px(290.)).child(sub)),
        )
        .child(body)
        .children(note);

    let top = h_flex()
        .justify_between()
        .px(px(space::XXL))
        .pt(px(space::L))
        .child(
            h_flex().gap(px(10.)).child(crate::app::mark(p)).child(div().typeset(text::HEADING).text_color(h.ink).child(crate::APP_NAME)),
        )
        .children(corner.map(|c| div().rounded(px(RADIUS_CONTROL)).bg(p.surface).child(c)));

    div()
        .id(id)
        .size_full()
        .relative()
        .text_color(p.text)
        .typeset(text::BODY)
        .bg(h_base(p))
        .child(img(crate::ui::pictures::hero(p.dark)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover))
        .child(
            v_flex().absolute().inset_0().child(top).child(
                div()
                    .id(SharedString::from(format!("{id}-scroll")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(v_flex().min_h_full().items_center().justify_center().py(px(space::XL)).child(card)),
            ),
        )
        .into_any_element()
}
