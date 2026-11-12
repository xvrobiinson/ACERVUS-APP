//! The Team section (roadmap 58, approved by the owner 2026-11-11:
//! https://claude.ai/artifact/JbAJr5H8wxLFWmujp8SQDe; the web's
//! src/ui/team.tsx): in windows 1,600px and wider, a column on the right of
//! every screen (it folds to a strip of faces, this PC's `team_folded`);
//! narrower, a people button by the bell opens the same list. Each person
//! shows their photo, role and whether they are online; a click opens what
//! you can do with them. Only for businesses with more than one person.
//! While the window is in front, the app says "I'm here" once a minute
//! (`team_beat`), and "gone" when it goes to the back (`team_leave`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use super::{Route, VzApp};
use crate::i18n::t;
use crate::services::team::{self, Seen};
use crate::sync::wire::{Need, Role, TeamMember, TeamStatus};
use crate::tr;
use crate::ui::components::icon;
use crate::ui::kit::Button;
use crate::ui::screens::role_label;
use crate::ui::theme::{Palette, Typeset as _, palette, space, text};
use crate::utils::dates::DateStyle;

/// What the Team section keeps while the app runs.
#[derive(Default)]
pub(crate) struct Crew {
    /// The row opened to show what you can do with that person (user id).
    pub open: Option<String>,
    /// Teammates' photos by address, once fetched.
    pub photos: HashMap<String, Arc<Image>>,
    /// Addresses fetched or being fetched, not asked for again.
    pub asked: HashSet<String>,
    /// Settings › Team opens this person's page (user id).
    pub wanted: Option<String>,
    /// Debug builds' ACERVUS_START=team: made-up people, for screenshots.
    pub demo: Option<Vec<TeamMember>>,
}

/// Made-up people for screenshots (the web demo's src/demo/team.ts).
pub(crate) fn demo_team() -> Vec<TeamMember> {
    let now = chrono::Local::now().timestamp_millis() as f64;
    let ago = |min: f64| now - min * 60_000.;
    let status = |online: bool, min: f64, device: &str| Some(TeamStatus { online, seen_at: ago(min), device: device.into() });
    let person = |id: &str, name: &str, role: Role, me: bool, phone: &str, s: Option<TeamStatus>| TeamMember {
        user_id: id.into(),
        name: name.into(),
        role,
        me,
        photo_url: None,
        phone: phone.into(),
        status: s,
    };
    vec![
        person("demo", "Demo", Role::Admin, true, "", status(true, 30., "pc")),
        person("demo-ana", "Ana Martis", Role::Collector, false, "+599 9 512 3456", status(true, 48., "phone")),
        person("demo-luis", "Luis Croes", Role::Manager, false, "+599 9 561 2200", status(false, 12., "pc")),
        person("demo-ramon", "Ramon Pieters", Role::Collector, false, "", status(false, 190., "phone")),
        person("demo-sofia", "Sofia Geerman", Role::Accountant, false, "", None),
    ]
}

fn seen_text(s: &Seen, ds: DateStyle) -> String {
    match s {
        Seen::Online => t("Online", "En línea").into(),
        Seen::JustNow => t("Seen just now", "Visto hace un momento").into(),
        Seen::Minutes(n) => tr!("Seen {n} min ago", "Visto hace {n} min"),
        Seen::Today(at) => tr!("Seen today at {at}", "Visto hoy a las {at}"),
        Seen::Yesterday(at) => tr!("Seen yesterday at {at}", "Visto ayer a las {at}"),
        Seen::On(d) => {
            let d = ds.format(*d);
            tr!("Seen {d}", "Visto el {d}")
        }
    }
}

/// A face: the photo or the initials, with a green dot while online.
fn face(p: &Palette, m: &TeamMember, photo: Option<Arc<Image>>, edge: Hsla) -> Div {
    let online = m.status.as_ref().is_some_and(|s| s.online);
    div()
        .relative()
        .flex_none()
        .size(px(40.))
        .child(
            h_flex()
                .size_full()
                .rounded_full()
                .overflow_hidden()
                .justify_center()
                .bg(p.accent_tint)
                .text_color(p.accent_text)
                .typeset(text::LABEL)
                .child(match photo {
                    Some(image) => img(image).size_full().object_fit(ObjectFit::Cover).into_any_element(),
                    None => div().child(super::initials_of(&m.name, "?")).into_any_element(),
                }),
        )
        .when(online, |d| {
            d.child(
                div().absolute().right(px(-1.)).bottom(px(-1.)).size(px(12.)).rounded_full().bg(p.positive).border_2().border_color(edge),
            )
        })
}

/// How many others are online.
fn online_count(list: &[TeamMember]) -> usize {
    list.iter().filter(|m| !m.me && m.status.as_ref().is_some_and(|s| s.online)).count()
}

impl VzApp {
    /// The team, in the list's order; None for a business of one (no Team
    /// section) or one this PC doesn't follow.
    pub(crate) fn team_list(&self) -> Option<Vec<TeamMember>> {
        if let Some(demo) = &self.crew.demo {
            return Some(team::order(demo));
        }
        let snap = self.cloud_copy()?;
        if !team::has_team(snap) {
            return None;
        }
        Some(team::order(&self.cloud.as_ref()?.team))
    }

    /// Whether there is a team to show, without building and sorting the
    /// list (asked on every frame by the page's width).
    fn shows_team(&self) -> bool {
        self.crew.demo.is_some() || self.cloud_copy().is_some_and(team::has_team)
    }

    /// The Team column's width: 0 when it doesn't show (narrow window, or
    /// no team).
    pub(crate) fn team_column_width(&self, window: &Window) -> f32 {
        if f32::from(window.viewport_size().width) < team::COLUMN_FROM || !self.shows_team() {
            0.
        } else if self.settings().team_folded {
            team::FOLDED_W
        } else {
            team::COLUMN_W
        }
    }

    /// "I'm here", while the window is in front, on a business with a team.
    pub(crate) fn team_beat(&self, window: &Window) {
        if !window.is_window_active() || self.crew.demo.is_some() {
            return;
        }
        let Some(snap) = self.cloud_copy().filter(|s| team::has_team(s)) else { return };
        drop(self.cloud_call("team:beat", json!({ "businessId": snap.business.id, "device": "pc" })));
    }

    /// Gone: the window went to the back.
    pub(crate) fn team_leave(&self) {
        let Some(snap) = self.cloud_copy().filter(|s| team::has_team(s)) else { return };
        drop(self.cloud_call("team:leave", json!({ "businessId": snap.business.id, "device": "pc" })));
    }

    /// Fetches the photos not fetched yet.
    fn team_photos(&mut self, list: &[TeamMember], window: &mut Window, cx: &mut Context<Self>) {
        for url in list.iter().filter_map(|m| m.photo_url.clone()) {
            if !self.crew.asked.insert(url.clone()) {
                continue;
            }
            cx.spawn_in(window, async move |this, cx| {
                let from = url.clone();
                match cx.background_spawn(async move { super::profile::fetch(&from) }).await {
                    Ok(bytes) => {
                        let _ = this.update(cx, |app, cx| {
                            app.crew.photos.insert(url, Arc::new(Image::from_bytes(ImageFormat::Jpeg, bytes)));
                            cx.notify();
                        });
                    }
                    Err(e) => log::info!("team photo not loaded: {e}"),
                }
            })
            .detach();
        }
    }

    /// The column on the right, in wide windows.
    pub(crate) fn render_team_column(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let width = self.team_column_width(window);
        if width == 0. {
            return None;
        }
        let list = self.team_list()?;
        self.team_photos(&list, window, cx);
        let p = palette(cx);
        let folded = self.settings().team_folded;
        let fold = Button::new("team-fold")
            .ghost()
            .small()
            .icon(if folded { IconName::ChevronLeft } else { IconName::ChevronRight })
            .tooltip(if folded { t("Open the team", "Abrir el equipo") } else { t("Fold the team", "Plegar el equipo") })
            .on_click(cx.listener(|this, _, w, cx| this.update_settings(|s| s.team_folded = !s.team_folded, w, cx)));
        let head = h_flex()
            .flex_none()
            .gap(px(space::S))
            .map(|d| if folded { d.justify_center().pt(px(12.)).pb(px(6.)) } else { d.pl(px(18.)).pr(px(12.)).pt(px(14.)).pb(px(8.)) })
            .when(!folded, |d| d.child(self.team_title(&p, &list)).child(div().flex_1()))
            .child(fold);
        let body = if folded {
            let faces = list.iter().enumerate().map(|(i, m)| {
                let photo = m.photo_url.as_ref().and_then(|u| self.crew.photos.get(u).cloned());
                let name = SharedString::from(m.name.clone());
                div()
                    .id(("team-strip", i))
                    .rounded_full()
                    .cursor_pointer()
                    .child(face(&p, m, photo, p.surface))
                    .tooltip(move |w, cx| Tooltip::new(name.clone()).build(w, cx))
                    .on_click(cx.listener(|this, _, w, cx| this.update_settings(|s| s.team_folded = false, w, cx)))
            });
            div()
                .id("team-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(v_flex().items_center().gap(px(10.)).pb(px(12.)).children(faces))
        } else {
            let rows = team_rows(self, &p, cx.entity().downgrade(), "col");
            div().id("team-scroll").flex_1().min_h_0().overflow_y_scroll().child(v_flex().px(px(8.)).pb(px(12.)).children(rows))
        };
        Some(
            div()
                .flex_none()
                .w(px(width))
                .h_full()
                .py(px(16.))
                .pr(px(16.))
                .child(
                    v_flex()
                        .size_full()
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border)
                        .rounded(px(20.))
                        .overflow_hidden()
                        .child(head)
                        .child(body),
                )
                .into_any_element(),
        )
    }

    /// "Team" and how many others are online.
    fn team_title(&self, p: &Palette, list: &[TeamMember]) -> Div {
        let n = online_count(list);
        let live = self.crew.demo.is_some() || self.cloud.as_ref().is_some_and(|c| c.online);
        v_flex().min_w_0().gap(px(2.)).child(div().typeset(text::HEADING).text_color(p.text).child(t("Team", "Equipo"))).when(live, |d| {
            d.child(div().typeset(text::CAPTION).text_color(p.muted).child(if n == 0 {
                t("Nobody else online", "Nadie más en línea").to_string()
            } else {
                tr!("{n} online", "{n} en línea")
            }))
        })
    }

    /// The people button by the bell, where the column doesn't show.
    pub(crate) fn team_button(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.team_column_width(window) > 0. {
            return None;
        }
        let list = self.team_list()?;
        self.team_photos(&list, window, cx);
        let p = palette(cx);
        let h = p.hero();
        let n = online_count(&list);
        let tip = tr!("Team, {n} online", "Equipo, {n} en línea");
        Some(
            h_flex()
                .id("team-head")
                .flex_none()
                .size(px(36.))
                .justify_center()
                .rounded_full()
                .bg(h.glass)
                .border_1()
                .border_color(h.glass_edge)
                .cursor_pointer()
                .hover(|d| d.bg(h.glass_hover))
                .relative()
                .child(icon(IconName::Users, h.ink, 15.))
                // Someone else online: a green dot on the corner.
                .when(n > 0, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(1.))
                            .right(px(1.))
                            .size(px(10.))
                            .rounded_full()
                            .bg(p.positive)
                            .border_2()
                            .border_color(p.ground),
                    )
                })
                .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                .on_click(cx.listener(|this, _, w, cx| this.open_team(w, cx)))
                .into_any_element(),
        )
    }

    /// The team in a window (narrower windows).
    pub(crate) fn open_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        let view = cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            TeamSheet { app: app.downgrade() }
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .entrance(gpui_kit::component::dialog::DialogEntrance::FadeSlide)
                .title(t("Team", "Equipo"))
                .w(px(480.))
                .child(view.clone())
        });
    }

    /// Opens Settings › Team on someone's page.
    fn open_team_settings(&mut self, user_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.crew.wanted = Some(user_id);
        self.team_view = None;
        self.navigate(Route::Settings, window, cx);
        self.settings_panel = Some(crate::ui::screens::settings::SettingsPanel::Team);
        cx.notify();
    }

    /// Clients, showing only those `user_id` collects from.
    fn open_their_clients(&mut self, user_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(
            |s| s.clients_filter = crate::services::list_filters::ClientsBy { collectors: vec![user_id], ..Default::default() },
            window,
            cx,
        );
        self.navigate(Route::Clients, window, cx);
    }
}

/// The people window (narrower windows): the column's list.
pub struct TeamSheet {
    app: WeakEntity<VzApp>,
}

impl Render for TeamSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let Some(app) = self.app.upgrade() else { return div().into_any_element() };
        let rows = team_rows(app.read(cx), &p, self.app.clone(), "sheet");
        v_flex().pb(px(space::S)).children(rows).into_any_element()
    }
}

/// The list's rows, for the column (`place` "col") and the window ("sheet").
fn team_rows(a: &VzApp, p: &Palette, app: WeakEntity<VzApp>, place: &'static str) -> Vec<AnyElement> {
    let Some(list) = a.team_list() else { return Vec::new() };
    if list.is_empty() {
        let note = if a.cloud.as_ref().is_some_and(|c| c.online) {
            t("Loading the team…", "Cargando el equipo…")
        } else {
            t("The team shows when you're online.", "El equipo se muestra cuando estás en línea.")
        };
        return vec![div().px(px(10.)).py(px(space::S)).typeset(text::CAPTION).text_color(p.muted).child(note).into_any_element()];
    }
    let now = chrono::Local::now();
    let ds = a.settings().date_style;
    let money = a.settings().money();
    let today = a.today;
    let sheet = place == "sheet";
    list.iter()
        .enumerate()
        .map(|(i, m)| {
            let open = a.crew.open.as_deref() == Some(m.user_id.as_str());
            let photo = m.photo_url.as_ref().and_then(|u| a.crew.photos.get(u).cloned());
            let seen = if m.me { None } else { m.status.as_ref().map(|s| (seen_text(&team::seen(s, now), ds), s.online)) };
            let brought = (a.can(Need::PaymentsConfirm) && m.role == Role::Collector)
                .then(|| a.cloud_copy().map(|snap| team::collected_today(snap, &m.user_id, today)))
                .flatten();
            let who = &m.name;
            let name = if m.me { tr!("{who} (you)", "{who} (tú)") } else { m.name.clone() };
            let device = (!m.me).then(|| m.status.as_ref().map(|s| s.device == "phone")).flatten();
            let id = m.user_id.clone();
            let toggle = app.clone();
            let row = h_flex()
                .id(SharedString::from(format!("team-{place}-{i}")))
                .w_full()
                .gap(px(12.))
                .p(px(10.))
                .rounded(px(14.))
                .cursor_pointer()
                .when(!open, |d| d.hover(|d| d.bg(p.hover)))
                .child(face(p, m, photo, if open { p.raised } else { p.surface }))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(1.))
                        .child(div().typeset(text::BODY_STRONG).text_color(p.text).truncate().child(name))
                        .child(h_flex().flex_wrap().typeset(text::CAPTION).text_color(p.muted).child(role_label(m.role)).when_some(
                            seen,
                            |d, (words, online)| {
                                d.child(div().child("\u{00a0}·\u{00a0}"))
                                    .child(div().when(online, |d| d.text_color(p.positive)).child(words))
                            },
                        ))
                        .when_some(brought, |d, (n, amount)| {
                            let words = if n > 0 {
                                let amount = money.format(amount);
                                tr!("{n} collected today · {amount}", "{n} cobrados hoy · {amount}")
                            } else {
                                t("Nothing collected today", "Nada cobrado hoy").to_string()
                            };
                            d.child(div().typeset(text::CAPTION).text_color(p.text).child(words))
                        }),
                )
                .when_some(device, |d, phone| {
                    let tip = if phone { t("On a phone", "En un teléfono") } else { t("On a PC", "En una PC") };
                    d.child(
                        div()
                            .id(SharedString::from(format!("team-{place}-dev-{i}")))
                            .flex_none()
                            .child(icon(if phone { IconName::Smartphone } else { IconName::Monitor }, p.faint, 15.))
                            .tooltip(move |w, cx| Tooltip::new(tip).build(w, cx)),
                    )
                })
                .on_click(move |_, _, cx| {
                    let id = id.clone();
                    let _ = toggle.update(cx, |app, cx| {
                        app.crew.open = if app.crew.open.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                        cx.notify();
                    });
                });
            v_flex()
                .rounded(px(14.))
                .when(open, |d| d.bg(p.raised))
                .child(row)
                .when(open, |d| d.child(person_actions(a, p, m, app.clone(), place, sheet)))
                .into_any_element()
        })
        .collect()
}

/// What you can do with someone: copy their number or WhatsApp them (when
/// they gave a phone), see the clients they collect from, open their page in
/// Settings › Team; Edit profile on your own row.
fn person_actions(a: &VzApp, p: &Palette, m: &TeamMember, app: WeakEntity<VzApp>, place: &'static str, sheet: bool) -> Div {
    let bar = h_flex().flex_wrap().gap(px(space::S)).pl(px(62.)).pr(px(10.)).pb(px(12.));
    let close = move |w: &mut Window, cx: &mut App| {
        if sheet {
            w.close_dialog(cx);
        }
    };
    let key = |what: &str| SharedString::from(format!("team-{place}-{what}"));
    if m.me {
        let to = app.clone();
        return bar.child(Button::new(key("profile")).small().icon(IconName::Pencil).label(t("Edit profile", "Editar perfil")).on_click(
            move |_, w, cx| {
                close(w, cx);
                let _ = to.update(cx, |app, cx| app.open_profile(w, cx));
            },
        ));
    }
    let currency = a.settings().currency_code.clone();
    let wa = (!m.phone.is_empty()).then(|| crate::services::whatsapp::phone_for_link(&m.phone, &currency)).flatten();
    // Only where Clients can filter by collector (Owner, Admin, Manager):
    // an Accountant would get every client under a collector's name.
    let clients = m.role == Role::Collector && a.manages_collectors();
    let settings = a.can(Need::TeamCollectors) && m.role != Role::Owner && (m.role == Role::Collector || a.can(Need::Team));
    let phone = m.phone.clone();
    let id = m.user_id.clone();
    let mut out = bar;
    if !phone.is_empty() {
        let to = app.clone();
        out = out.child(Button::new(key("copy")).small().icon(IconName::Copy).label(t("Copy number", "Copiar número")).on_click(
            move |_, w, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(phone.clone()));
                let _ = to.update(cx, |app, cx| app.notify_success(t("Number copied", "Número copiado"), w, cx));
            },
        ));
    }
    if let Some(wa) = wa {
        let to = app.clone();
        out = out.child(
            Button::new(key("wa"))
                .small()
                .prefix(img(crate::ui::pictures::whatsapp()).size(px(15.)))
                .label(t("WhatsApp", "WhatsApp"))
                .on_click(move |_, w, cx| {
                    let _ = to.update(cx, |app, cx| app.open_whatsapp(format!("https://wa.me/{wa}"), w, cx));
                }),
        );
    }
    if clients {
        let (to, id) = (app.clone(), id.clone());
        out = out.child(Button::new(key("clients")).small().icon(IconName::Users).label(t("Their clients", "Sus clientes")).on_click(
            move |_, w, cx| {
                close(w, cx);
                let _ = to.update(cx, |app, cx| app.open_their_clients(id.clone(), w, cx));
            },
        ));
    }
    if settings {
        let to = app.clone();
        out = out.child(
            Button::new(key("settings")).small().ghost().icon(IconName::Settings).label(t("Team settings", "Ajustes del equipo")).on_click(
                move |_, w, cx| {
                    close(w, cx);
                    let _ = to.update(cx, |app, cx| app.open_team_settings(id.clone(), w, cx));
                },
            ),
        );
    }
    if m.phone.is_empty() && !clients && !settings {
        out = out.child(
            div().typeset(text::CAPTION).text_color(p.muted).child(t("No phone number in their profile.", "No hay teléfono en su perfil.")),
        );
    }
    out
}
