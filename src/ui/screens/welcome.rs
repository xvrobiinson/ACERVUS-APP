//! The business welcome: what the app shows when signed in with no business
//! open on this PC (the app always shows a cloud business, never books kept
//! only here). Waiting for the list of businesses, creating the first one
//! or accepting an invitation, or choosing among several. The same sky and
//! glass card as the sign-in screen.

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::sign_in::{GlassCard, glass_screen};
use crate::app::{Gate, VzApp};
use crate::i18n::t;
use crate::tr;
use crate::ui::kit::Button;
use crate::ui::theme::{RADIUS_CONTROL, Typeset as _, palette, space, text};

impl VzApp {
    pub(crate) fn render_gate(&mut self, gate: Gate, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let Some(c) = &self.cloud else { return div().into_any_element() };
        let email = c.email.clone();
        let sign_out = Button::new("gate-sign-out")
            .ghost()
            .label(t("Sign out", "Cerrar sesión"))
            .on_click(cx.listener(|this, _, w, cx| this.sign_out(w, cx)))
            .into_any_element();
        // A boxed line inside the card: words on the left, a button on the right.
        let line = |id: SharedString, words: AnyElement, button: Button| {
            h_flex()
                .id(id)
                .gap(px(space::M))
                .items_center()
                .p(px(space::M))
                .rounded(px(RADIUS_CONTROL))
                .border_1()
                .border_color(p.border)
                .child(div().flex_1().min_w_0().child(words))
                .child(button)
                .into_any_element()
        };

        let card = match gate {
            Gate::Waiting => GlassCard {
                glyph: IconName::Cloud,
                title: t("Getting your businesses…", "Buscando tus negocios…").to_string(),
                sub: if c.online {
                    tr!("Signed in as {email}", "Sesión iniciada como {email}")
                } else {
                    t(
                        "Waiting for a connection. The first time on this PC needs the internet to find your businesses.",
                        "Esperando conexión. La primera vez en esta PC necesita internet para encontrar tus negocios.",
                    )
                    .to_string()
                },
                body: div().into_any_element(),
                note: None,
            },
            Gate::Welcome => {
                let invites = c.invites.clone().unwrap_or_default().into_iter().map(|i| {
                    let (name, role) = (i.business_name.clone(), super::sync::role_label(i.role));
                    let busy = c.accepting.as_deref() == Some(i.invite_id.as_str());
                    let id = i.invite_id.clone();
                    line(
                        SharedString::from(format!("gate-invite-{}", i.invite_id)),
                        div().child(tr!("You were invited to {name} as {role}.", "Te invitaron a {name} como {role}.")).into_any_element(),
                        Button::new(SharedString::from(format!("gate-accept-{}", i.invite_id)))
                            .primary()
                            .label(t("Accept", "Aceptar"))
                            .loading(busy)
                            .on_click(cx.listener(move |this, _, w, cx| this.accept_invite(id.clone(), w, cx))),
                    )
                });
                GlassCard {
                    glyph: IconName::Landmark,
                    title: t("Create your business", "Crea tu negocio").to_string(),
                    sub: t(
                        "You'll be its administrator. You can invite others later.",
                        "Serás su administrador. Luego puedes invitar a otros.",
                    )
                    .to_string(),
                    body: v_flex()
                        .gap(px(space::M))
                        .children(invites)
                        .child(
                            Button::new("gate-new-business")
                                .primary()
                                .icon(IconName::Plus)
                                .label(t("Create your business", "Crea tu negocio"))
                                .wide()
                                .disabled(!c.online)
                                .on_click(cx.listener(|this, _, w, cx| this.open_new_business(w, cx))),
                        )
                        .into_any_element(),
                    note: Some(
                        div()
                            .text_center()
                            .typeset(text::CAPTION)
                            .text_color(p.faint)
                            .child(tr!("Invitations sent to {email} show here.", "Las invitaciones enviadas a {email} aparecen aquí."))
                            .into_any_element(),
                    ),
                }
            }
            Gate::Choose => {
                let rows = c.businesses.clone().unwrap_or_default().into_iter().map(|b| {
                    let id = b.business_id.clone();
                    line(
                        SharedString::from(format!("gate-business-{}", b.business_id)),
                        v_flex()
                            .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(b.name.clone()))
                            .child(div().typeset(text::CAPTION).text_color(p.muted).child(super::sync::role_label(b.role)))
                            .into_any_element(),
                        Button::new(SharedString::from(format!("gate-open-{}", b.business_id)))
                            .primary()
                            .label(t("Open", "Abrir"))
                            .on_click(cx.listener(move |this, _, w, cx| this.link_business(&id, w, cx))),
                    )
                });
                GlassCard {
                    glyph: IconName::Landmark,
                    title: t("Choose a business", "Elige un negocio").to_string(),
                    sub: t("You can switch any time from the sidebar.", "Puedes cambiar cuando quieras desde el menú lateral.").to_string(),
                    body: v_flex().gap(px(space::S)).children(rows).into_any_element(),
                    note: None,
                }
            }
        };
        glass_screen(&p, w, h, "business-gate", card, Some(sign_out))
    }
}
