//! Settings › Team: an admin's view of who works in the business. The list
//! groups people by role; a person's page changes their role, removes them,
//! and for a collector chooses what they see of the clients assigned to
//! them. Invitations are made from the list. It talks to the cloud directly
//! (these aren't changes to the books), so it needs a connection.

use futures::channel::oneshot;
use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::VzApp;
use crate::i18n::t;
use crate::sync::outbox::Outcome;
use crate::tr;
use crate::ui::components::{Tone, icon, pill, sel};
use crate::ui::kit::{Button, Dropdown, DropdownItem, ListRow};
use crate::ui::theme::{Palette, Typeset as _, palette, space, text};

/// A person in the business (`businesses:members`).
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    #[serde(rename = "_id")]
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub active: bool,
    #[serde(default)]
    pub collector_view: Option<serde_json::Map<String, Value>>,
}

/// An invitation not used yet (`invites:list`).
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    #[serde(rename = "_id")]
    pub id: String,
    pub email: String,
    pub role: String,
}

/// A collector's switches: the cloud's key and its label.
fn view_parts() -> [(&'static str, &'static str); 7] {
    [
        ("amountLent", t("Amount lent", "Monto prestado")),
        ("rate", t("Interest rate", "Tasa de interés")),
        ("contact", t("Client phone and address", "Teléfono y dirección del cliente")),
        ("idNumber", t("Client ID number", "Cédula del cliente")),
        ("history", t("Payment history", "Historial de pagos")),
        ("lateFees", t("Late fees", "Mora")),
        ("notes", t("Client notes", "Notas del cliente")),
    ]
}

/// A switch's state: a missing one is on.
fn view_on(m: &Member, key: &str) -> bool {
    m.collector_view.as_ref().and_then(|v| v.get(key)).and_then(Value::as_bool).unwrap_or(true)
}

/// The whole set of switches with one changed, as the cloud takes it.
fn view_with(m: &Member, key: &str, on: bool) -> Value {
    let all: serde_json::Map<String, Value> =
        view_parts().iter().map(|(k, _)| (k.to_string(), Value::Bool(if *k == key { on } else { view_on(m, k) }))).collect();
    Value::Object(all)
}

fn role_label(role: &str) -> &'static str {
    match role {
        "admin" => t("Admin", "Administrador"),
        "collector" => t("Collector", "Cobrador"),
        _ => t("Viewer", "Solo lectura"),
    }
}

const ROLES: [&str; 3] = ["admin", "collector", "viewer"];

/// A role's group heading, its pill's tone and its icon on the list.
fn role_look(role: &str) -> (&'static str, Tone, IconName) {
    match role {
        "admin" => (t("Admins", "Administradores"), Tone::Accent, IconName::ShieldCheck),
        "collector" => (t("Collectors", "Cobradores"), Tone::Positive, IconName::HandCoins),
        _ => (t("Viewers", "Solo lectura"), Tone::Neutral, IconName::Eye),
    }
}

/// A heading over a group of people.
fn group_title(p: &Palette, title: &str) -> Div {
    div().pt(px(space::S)).typeset(text::CAPTION).font_weight(FontWeight::SEMIBOLD).text_color(p.muted).child(title.to_uppercase())
}

pub struct TeamView {
    app: WeakEntity<VzApp>,
    business: Option<String>,
    me: String,
    members: Option<Vec<Member>>,
    invites: Vec<Invite>,
    email: Entity<InputState>,
    role: &'static str,
    /// The invitation just made: its link, and whether it was emailed.
    link: Option<(String, bool)>,
    /// The person whose page is open.
    open: Option<String>,
    /// The person being asked about before removing them.
    removing: Option<String>,
    busy: bool,
    error: Option<String>,
}

impl TeamView {
    /// Built while the app draws, so what it needs from the app comes as
    /// arguments and the first read of the team waits until after that.
    pub fn new(app: WeakEntity<VzApp>, business: Option<String>, me: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let email = cx.new(|cx| InputState::new(window, cx).placeholder(t("Email address", "Correo electrónico")));
        cx.defer_in(window, |view, window, cx| view.load(window, cx));
        Self {
            app,
            business,
            me,
            members: None,
            invites: Vec::new(),
            email,
            role: "collector",
            link: None,
            open: None,
            removing: None,
            busy: false,
            error: None,
        }
    }

    /// The name over the open person's page, if one is open.
    pub fn person_title(&self) -> Option<SharedString> {
        let id = self.open.as_ref()?;
        self.members.iter().flatten().find(|m| &m.id == id && m.active).map(|m| m.name.clone().into())
    }

    /// Back from a person's page to the list; false when the list is showing.
    pub fn back(&mut self, cx: &mut Context<Self>) -> bool {
        if self.open.take().is_none() {
            return false;
        }
        self.removing = None;
        self.error = None;
        cx.notify();
        true
    }

    fn app_read<R>(&self, cx: &App, f: impl FnOnce(&VzApp) -> R) -> Option<R> {
        self.app.upgrade().map(|a| f(a.read(cx)))
    }

    /// Reads the team and the open invitations again.
    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.business.clone() else { return };
        let ask = self.app_read(cx, |a| {
            (a.cloud_query("businesses:members", json!({ "businessId": id })), a.cloud_query("invites:list", json!({ "businessId": id })))
        });
        let Some((Some(members), Some(invites))) = ask else { return };
        cx.spawn_in(window, async move |this, cx| {
            let (members, invites) = (members.await.unwrap_or(Outcome::NotSent), invites.await.unwrap_or(Outcome::NotSent));
            let _ = this.update_in(cx, |view, _, cx| {
                match (members, invites) {
                    (Outcome::Accepted(m), Outcome::Accepted(i)) => {
                        view.members = serde_json::from_value::<Vec<Member>>(m).ok();
                        view.invites = serde_json::from_value(i).unwrap_or_default();
                        // Someone removed (here or elsewhere) has no page.
                        let gone = |id: &String| !view.members.iter().flatten().any(|m| &m.id == id && m.active);
                        if view.open.as_ref().is_some_and(gone) {
                            view.open = None;
                        }
                    }
                    (other, _) => view.error = view.app_read(cx, |a| a.call_failed(&other)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Sends a call, shows what went wrong if it did, then reads the team again.
    fn send(
        &mut self,
        answer: Option<oneshot::Receiver<Outcome>>,
        then: impl FnOnce(&mut Self, &Value) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.error = None;
        cx.spawn_in(window, async move |this, cx| {
            let outcome = match answer {
                Some(a) => a.await.unwrap_or(Outcome::NotSent),
                None => Outcome::NotSent,
            };
            let _ = this.update_in(cx, |view, window, cx| {
                view.busy = false;
                match &outcome {
                    Outcome::Accepted(result) => then(view, result),
                    other => view.error = view.app_read(cx, |a| a.call_failed(other)),
                }
                view.load(window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn mutate(&mut self, mutation: &'static str, mut args: Value, window: &mut Window, cx: &mut Context<Self>) {
        args["businessId"] = json!(self.business);
        let answer = self.app_read(cx, |a| a.cloud_call(mutation, args)).flatten();
        self.send(answer, |_, _| {}, window, cx);
    }

    fn invite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let email = self.email.read(cx).value().trim().to_string();
        if !email.contains('@') {
            self.error = Some(t("Enter a valid email address", "Escribe un correo válido").to_string());
            cx.notify();
            return;
        }
        let args = json!({ "businessId": self.business, "email": email, "role": self.role });
        let answer = self.app_read(cx, |a| a.cloud_action("invites:create", args)).flatten();
        self.send(
            answer,
            |view, result| {
                view.link = result["url"].as_str().map(|u| (u.to_string(), result["emailed"] == true));
            },
            window,
            cx,
        );
        self.email.update(cx, |s, cx| s.set_value(String::new(), window, cx));
    }

    /// A person on the list: name, email and role; opens their page.
    fn person_row(&self, p: &Palette, m: &Member, cx: &mut Context<Self>) -> ListRow {
        let me = m.user_id == self.me;
        let name = if me { format!("{} · {}", m.name, t("you", "tú")) } else { m.name.clone() };
        let (_, tone, mark) = role_look(&m.role);
        let id = m.id.clone();
        ListRow::new(SharedString::from(format!("team-person-{}", m.id)))
            .boxed(true)
            .leading(icon(mark, p.muted, 18.))
            .title(name)
            .subtitle(m.email.clone())
            .trailing(h_flex().gap(px(space::S)).child(pill(p, role_label(&m.role), tone)).child(icon(
                IconName::ChevronRight,
                p.muted,
                16.,
            )))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.open = Some(id.clone());
                view.removing = None;
                view.error = None;
                cx.notify();
            }))
    }

    /// A person's page: their role, what a collector sees, and removing them.
    fn person_page(&self, p: &Palette, m: &Member, cx: &mut Context<Self>) -> Div {
        let member = m.id.clone();
        let menu =
            Dropdown::new(SharedString::from(format!("team-role-{}", m.id))).label(role_label(&m.role)).align_end().items(ROLES.map(|r| {
                let id = member.clone();
                DropdownItem::new(
                    role_label(r),
                    cx.listener(move |view, _, w, cx| view.mutate("businesses:setRole", json!({ "memberId": id, "role": r }), w, cx)),
                )
                .selected(m.role == r)
            }));
        let asking = self.removing.as_deref() == Some(m.id.as_str());
        let (remove_id, confirm_id) = (member.clone(), member.clone());
        v_flex()
            .gap(px(space::L))
            .child(div().typeset(text::CAPTION).text_color(p.muted).child(sel(m.email.clone())))
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap(px(space::M))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(240.))
                            .gap(px(2.))
                            .child(div().typeset(text::BODY_STRONG).child(t("Role", "Rol")))
                            .child(div().typeset(text::CAPTION).text_color(p.muted).child(t(
                                "Admin: everything. Collector: their assigned clients, payments, expenses, income and receipts. Viewer: read only.",
                                "Administrador: todo. Cobrador: sus clientes asignados, pagos, gastos, ingresos y recibos. Solo lectura: ver.",
                            ))),
                    )
                    .child(menu),
            )
            .when(m.role == "collector", |d| d.child(self.permissions(p, m, cx)))
            .child(
                v_flex()
                    .gap(px(space::S))
                    .pt(px(space::M))
                    .border_t_1()
                    .border_color(p.rule)
                    .child(div().typeset(text::CAPTION).text_color(p.muted).child(t(
                        "They lose access to this business at once. Their past changes stay in the history.",
                        "Pierde el acceso al negocio de inmediato. Sus cambios anteriores siguen en el historial.",
                    )))
                    .child(if asking {
                        h_flex()
                            .gap(px(space::S))
                            .child(Button::new("team-remove-no").label(t("Cancel", "Cancelar")).on_click(cx.listener(|view, _, _, cx| {
                                view.removing = None;
                                cx.notify();
                            })))
                            .child(Button::new("team-remove-yes").danger().icon(IconName::Trash).label(t("Remove", "Quitar")).loading(self.busy).on_click(
                                cx.listener(move |view, _, w, cx| {
                                    view.removing = None;
                                    view.mutate("businesses:removeMember", json!({ "memberId": confirm_id, "reason": "" }), w, cx);
                                }),
                            ))
                    } else {
                        h_flex().child(
                            Button::new("team-remove")
                                .danger_ghost()
                                .icon(IconName::Trash)
                                .label(t("Remove from the team", "Quitar del equipo"))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.removing = Some(remove_id.clone());
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }

    /// What a collector sees of their assigned clients, one checkbox each.
    /// The cloud stores and enforces them; this only sends the choice.
    fn permissions(&self, p: &Palette, m: &Member, cx: &mut Context<Self>) -> Div {
        let boxes = view_parts().map(|(key, label)| {
            let member = m.clone();
            Checkbox::new(SharedString::from(format!("team-view-{}-{key}", m.id))).label(label).checked(view_on(m, key)).on_click(
                cx.listener(move |view, on: &bool, w, cx| {
                    let args = json!({ "memberId": member.id, "view": view_with(&member, key, *on) });
                    view.mutate("businesses:setCollectorView", args, w, cx);
                }),
            )
        });
        v_flex()
            .gap(px(space::S))
            .child(div().typeset(text::BODY_STRONG).child(t("Sees, for their assigned clients", "Ve, de sus clientes asignados")))
            .child(v_flex().gap(px(space::S)).children(boxes))
            .child(div().typeset(text::CAPTION).text_color(p.faint).child(t(
                "Phone, address, ID and notes never reach their device when off. The others are hidden on their screens and receipts.",
                "Teléfono, dirección, cédula y notas no llegan a su dispositivo si están apagados. Los demás se ocultan en sus pantallas y recibos.",
            )))
    }

    /// Everyone by role, then inviting and the open invitations.
    fn list(&self, p: &Palette, members: &[Member], cx: &mut Context<Self>) -> Div {
        let mut groups = v_flex().gap(px(space::S));
        for role in ROLES {
            let people: Vec<&Member> = members.iter().filter(|m| m.role == role).collect();
            // Every business has an admin; collectors and viewers show even when empty.
            if people.is_empty() && role == "admin" {
                continue;
            }
            groups = groups.child(group_title(p, role_look(role).0));
            groups = if people.is_empty() {
                groups.child(div().typeset(text::CAPTION).text_color(p.faint).child(t("No one yet", "Nadie todavía")))
            } else {
                groups.children(people.into_iter().map(|m| self.person_row(p, m, cx)))
            };
        }
        let invites = self.invites.clone();
        let role_menu = Dropdown::new("team-new-role").label(role_label(self.role)).items(ROLES.map(|r| {
            DropdownItem::new(
                role_label(r),
                cx.listener(move |view, _, _, cx| {
                    view.role = r;
                    cx.notify();
                }),
            )
            .selected(self.role == r)
        }));
        v_flex()
            .gap(px(space::M))
            .child(groups)
            .child(div().pt(px(space::S)).typeset(text::BODY_STRONG).child(t("Invite someone", "Invitar a alguien")))
            .child(
                h_flex().gap(px(space::S)).child(div().flex_1().min_w_0().child(Input::new(&self.email))).child(role_menu).child(
                    Button::new("team-invite")
                        .primary()
                        .icon(IconName::Mail)
                        .label(t("Send invitation", "Enviar invitación"))
                        .loading(self.busy)
                        .on_click(cx.listener(|view, _, w, cx| view.invite(w, cx))),
                ),
            )
            .when_some(self.link.clone(), |d, (url, emailed)| {
                d.child(
                    v_flex()
                        .gap(px(space::XS))
                        .child(div().typeset(text::CAPTION).text_color(p.muted).child(if emailed {
                            t(
                                "Invitation emailed. You can also share this link (valid 7 days, works once):",
                                "Invitación enviada. También puedes compartir este enlace (válido 7 días, funciona una vez):",
                            )
                        } else {
                            t("Share this link (valid 7 days, works once):", "Comparte este enlace (válido 7 días, funciona una vez):")
                        }))
                        .child(div().typeset(text::CAPTION).child(sel(url.clone())))
                        .child(
                            div().child(
                                Button::new("team-copy").small().label(t("Copy link", "Copiar enlace")).on_click(
                                    cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(url.clone()))),
                                ),
                            ),
                        ),
                )
            })
            .children(invites.into_iter().map(|i| {
                let (id, role) = (i.id.clone(), role_label(&i.role));
                let line = tr!("{role}, invited", "{role}, invitado");
                h_flex()
                    .py(px(space::S))
                    .gap(px(space::M))
                    .border_b_1()
                    .border_color(p.rule)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(sel(i.email.clone())))
                            .child(div().typeset(text::CAPTION).text_color(p.muted).child(line)),
                    )
                    .child(pill(p, t("Invited", "Invitado"), Tone::Neutral))
                    .child(
                        Button::new(SharedString::from(format!("team-revoke-{}", i.id)))
                            .ghost()
                            .small()
                            .label(t("Revoke", "Revocar"))
                            .on_click(cx.listener(move |view, _, w, cx| view.mutate("invites:revoke", json!({ "inviteId": id }), w, cx))),
                    )
            }))
    }
}

impl Render for TeamView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let members: Vec<Member> = self.members.iter().flatten().filter(|m| m.active).cloned().collect();
        let open = self.open.as_ref().and_then(|id| members.iter().find(|m| &m.id == id)).cloned();
        v_flex()
            .gap(px(space::M))
            .when_some(self.error.clone(), |d, e| d.child(div().text_color(p.negative).child(sel(e))))
            .when(self.members.is_none() && self.error.is_none(), |d| d.child(div().text_color(p.muted).child(t("Loading…", "Cargando…"))))
            .map(|d| match &open {
                Some(m) => d.child(self.person_page(&p, m, cx)),
                None => d.when(self.members.is_some(), |d| d.child(self.list(&p, &members, cx))),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    fn member(view: Option<Value>) -> Member {
        let mut m =
            json!({ "_id": "m1", "userId": "u", "name": "Ana", "email": "a@x", "role": "collector", "active": true, "joinedAt": 1 });
        if let Some(v) = view {
            m["collectorView"] = v;
        }
        serde_json::from_value(m).unwrap()
    }

    #[test]
    fn switches_start_on_and_send_the_whole_set() {
        let m = member(None);
        assert!(view_on(&m, "rate"));
        let sent = view_with(&m, "rate", false);
        assert_eq!(sent["rate"], false);
        assert_eq!(sent["notes"], true);
        assert_eq!(sent.as_object().unwrap().len(), 7);
        let m = member(Some(json!({ "rate": false })));
        assert!(!view_on(&m, "rate"));
        assert_eq!(view_with(&m, "notes", false)["rate"], false, "other switches keep their state");
    }
}
