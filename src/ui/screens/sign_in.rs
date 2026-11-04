//! The sign-in screen: a sky (night in dark mode) with soft clouds and two
//! faint arcs, the brand in the corner and one glass card holding the email
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
use crate::ui::kit::interact::faded;
use crate::ui::theme::{Elevation, Palette, RADIUS_CONTROL, Typeset as _, elevation, palette, space, text};

const CARD_W: f32 = 380.0;

/// Google's "G" in its own colors, as Google's sign-in branding asks.
static GOOGLE_G: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Svg, include_bytes!("../../../assets/google-g.svg").to_vec())));

/// Soft cloud puffs along the bottom: (center across the width 0–1, height
/// of the center above the bottom edge, diameter).
const PUFFS: [(f32, f32, f32); 9] = [
    (0.02, 40., 260.),
    (0.16, -10., 300.),
    (0.30, 30., 220.),
    (0.44, -40., 340.),
    (0.58, 10., 260.),
    (0.72, -30., 320.),
    (0.86, 40., 240.),
    (0.98, -20., 300.),
    (0.50, -120., 520.),
];

/// The sky behind the card (and, in dark mode, behind every screen),
/// `w` × `h` px.
pub(crate) fn sky(p: &Palette, w: f32, h: f32) -> Div {
    let (cx_, cy) = (w / 2., h * 0.58);
    let arc =
        |d: f32| div().absolute().left(px(cx_ - d / 2.)).top(px(cy - d / 2.)).size(px(d)).rounded_full().border_1().border_color(p.arc);
    let puffs = PUFFS.iter().map(|&(x, lift, d)| {
        div()
            .absolute()
            .left(px(x * w - d / 2.))
            .top(px(h - lift - d / 2.))
            .size(px(d))
            .rounded_full()
            .shadow(vec![BoxShadow::new(px(0.), px(0.), p.cloud).blur_radius(px(60.)).spread_radius(px(20.))])
    });
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .bg(linear_gradient(180., linear_color_stop(p.sky_top, 0.), linear_color_stop(p.sky_low, 1.)))
        .child(arc(1500.))
        .child(arc(1920.))
        .children(puffs)
}

/// The sign-in sky's life: stars that twinkle at night (dark mode) and a
/// few clouds drifting across by day (light mode). Only behind the sign-in
/// card; the sky behind the screens stays still.
fn sky_life(p: &Palette, w: f32, h: f32) -> Div {
    use std::time::Duration;
    // A repeating animation that starts `phase` of the way through.
    let looping = |secs: f32, phase: f32| {
        Animation::new(Duration::from_secs_f32(secs)).repeat().with_max_fps(30.).with_easing(move |t| (t + phase).fract())
    };
    let layer = div().absolute().inset_0();
    if p.dark {
        // Spread by a fixed pattern so the sky looks the same each time.
        let stars = (0..44u32).map(|i| {
            let x = ((i * 37 + 11) % 100) as f32 / 100.;
            let y = ((i * 53 + 7) % 64) as f32 / 100.;
            let size = if i % 7 == 0 { 3. } else { 2. };
            div().absolute().left(px(x * w)).top(px(y * h)).size(px(size)).rounded_full().bg(gpui_kit::white()).with_animation(
                ("star", i as u64),
                looping(2.6 + (i % 5) as f32 * 0.5, (i % 9) as f32 / 9.),
                |d, t| d.opacity(0.25 + 0.7 * (0.5 - 0.5 * (t * std::f32::consts::TAU).cos())),
            )
        });
        return layer.children(stars);
    }
    // (height down the sky 0–1, width, seconds to cross, start 0–1)
    const DRIFT: [(f32, f32, f32, f32); 4] =
        [(0.14, 180., 80., 0.1), (0.3, 120., 64., 0.55), (0.22, 220., 96., 0.8), (0.42, 140., 72., 0.3)];
    let clouds = DRIFT.iter().enumerate().map(|(i, &(y, cw, secs, start))| {
        let travel = w + cw * 2.;
        // A flat base with three puffs on top, softened at the edges.
        let soft = vec![BoxShadow::new(px(0.), px(0.), faded(p.cloud, 0.6)).blur_radius(px(14.))];
        let puff = |left: f32, size: f32| {
            div().absolute().left(px(left * cw)).bottom(px(cw * 0.06)).size(px(size * cw)).rounded_full().bg(p.cloud).shadow(soft.clone())
        };
        div()
            .absolute()
            .top(px(y * h))
            .w(px(cw))
            .h(px(cw * 0.5))
            .child(puff(0.08, 0.34))
            .child(puff(0.28, 0.46))
            .child(puff(0.56, 0.36))
            .child(div().absolute().left_0().right_0().bottom_0().h(px(cw * 0.2)).rounded_full().bg(p.cloud).shadow(soft.clone()))
            .with_animation(("cloud", i as u64), looping(secs, start), move |d, t| d.left(px(t * travel - cw)))
    });
    layer.children(clouds)
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
        .border_color(p.glass_edge)
        .shadow(elevation(p, Elevation::Raised))
        .child(icon(name, p.text, 22.))
}

impl VzApp {
    pub(crate) fn render_sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
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
        glass_screen(&p, w, h, "sign-in", GlassCard { glyph, title, sub, body: form.into_any_element(), note }, back)
    }
}

/// One card on the sign-in sky: a tile with an icon, a title, a line under
/// it, the card's content and an optional faint note at the foot.
pub(crate) struct GlassCard {
    pub glyph: IconName,
    pub title: String,
    pub sub: String,
    pub body: AnyElement,
    pub note: Option<AnyElement>,
}

/// The sign-in look: the sky (night in dark mode) with the brand in the
/// corner, an optional button in the other corner, and one glass card in
/// the middle. The business welcome uses it too.
pub(crate) fn glass_screen(p: &Palette, w: f32, h: f32, id: &'static str, card: GlassCard, corner: Option<AnyElement>) -> AnyElement {
    let GlassCard { glyph, title, sub, body, note } = card;
    let card = v_flex()
        .w(px(CARD_W.min(w - 2. * space::L)))
        .px(px(28.))
        .pt(px(32.))
        .pb(px(28.))
        .gap(px(space::L))
        .rounded(px(22.))
        .border_1()
        .border_color(p.glass_edge)
        .bg(linear_gradient(180., linear_color_stop(p.glass_top, 0.), linear_color_stop(p.glass, 0.4)))
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
            h_flex().gap(px(10.)).child(crate::app::mark(p)).child(div().typeset(text::HEADING).text_color(p.text).child(crate::APP_NAME)),
        )
        .children(corner.map(|c| div().rounded(px(RADIUS_CONTROL)).bg(faded(p.surface, 0.35)).child(c)));

    div()
        .id(id)
        .size_full()
        .relative()
        .text_color(p.text)
        .typeset(text::BODY)
        .child(sky(p, w, h).child(sky_life(p, w, h)))
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
