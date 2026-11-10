//! The top right of the window (approved 2026-11-05 with the redesign):
//! the light/dark pill, the bell, and the account icon whose ring shows how
//! the app is doing, most serious first, opening the account menu: who is
//! signed in, why the ring has its color, the businesses, Back up database
//! (admins), Sync options, Settings and Sign out. Every item opens what it
//! opened before; nothing here keeps state of its own but `account_open`.

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::cloud::SyncStatus;
use super::{Route, VzApp};
use crate::i18n::t;
use crate::services::settings::ThemePref;
use crate::sync::wire::{Need, Role};
use crate::tr;
use crate::ui::components::{Tone, icon, pill};
use crate::ui::kit::{Badge, BadgeTone, Button, Overlay, OverlayAlign, StatusAvatar, ThemePill};
use crate::ui::screens::role_label;
use crate::ui::theme::{Palette, RADIUS_CONTROL, Typeset as _, palette, space, text};

/// How serious, most serious first: 3 act now, 2 have a look, 1 all good,
/// 0 still checking.
pub(crate) type Severity = u8;

/// What a reason's button does.
#[derive(Clone, Copy)]
enum Fix {
    Settings,
    Payments,
    Update,
    Backup,
}

/// What a menu item does when picked.
type MenuAction = Box<dyn Fn(&mut VzApp, &mut Window, &mut Context<VzApp>)>;

struct Reason {
    sev: Severity,
    text: String,
    fix: Option<(&'static str, Fix)>,
}

/// A backup older than this needs a look.
const BACKUP_DAYS: i64 = 7;

impl VzApp {
    /// The app's state from real conditions only: sync, this version, the
    /// backups (admins, who can make one), collector payments to confirm
    /// (office staff). New messages are left out; the bell counts them.
    fn app_status(&self) -> (Severity, Vec<Reason>) {
        let mut out = Vec::new();
        let admin = self.can(Need::Manage);
        match self.sync_status() {
            None | Some(SyncStatus::NotLinked) => return (0, out),
            Some(SyncStatus::UpdateRequired) => out.push(Reason {
                sev: 3,
                text: t("This version is too old to sync", "Esta versión es muy vieja para sincronizar").into(),
                fix: Some((t("Update", "Actualizar"), Fix::Update)),
            }),
            Some(SyncStatus::NeedsReview(n)) => out.push(Reason {
                sev: 3,
                text: tr!("{n} change(s) were refused by the cloud", "La nube rechazó {n} cambio(s)"),
                fix: Some((t("Review", "Revisar"), Fix::Settings)),
            }),
            Some(SyncStatus::Offline(0)) => out.push(Reason { sev: 2, text: t("Offline", "Sin conexión").into(), fix: None }),
            Some(SyncStatus::Offline(n)) => out.push(Reason {
                sev: 2,
                text: tr!("Offline: {n} change(s) wait to be sent", "Sin conexión: {n} cambio(s) esperan"),
                fix: None,
            }),
            Some(SyncStatus::Synced | SyncStatus::Sending(_)) => {}
        }
        if admin {
            if let Some(e) = self.cloud.as_ref().and_then(|c| c.drive.error.clone()) {
                out.push(Reason { sev: 3, text: e, fix: Some((t("Open", "Abrir"), Fix::Settings)) });
            }
            let old = match &self.settings().last_backup_at {
                None => Some(t("Not backed up yet", "Sin copia de seguridad").to_string()),
                Some(when) => chrono::NaiveDateTime::parse_from_str(when, "%Y-%m-%d %H:%M").ok().and_then(|at| {
                    let days = (chrono::Local::now().naive_local() - at).num_days();
                    (days >= BACKUP_DAYS).then(|| tr!("Not backed up in {days} days", "Sin copia en {days} días"))
                }),
            };
            if let Some(text) = old {
                out.push(Reason { sev: 2, text, fix: Some((t("Back up", "Copiar"), Fix::Backup)) });
            }
        }
        if self.role() != Some(Role::Collector) {
            let n = self.cloud_copy().map_or(0, |s| s.pending.iter().filter(|p| p.status == "waiting").count());
            if n > 0 {
                out.push(Reason {
                    sev: 2,
                    text: tr!("{n} collector payment(s) to confirm", "{n} pago(s) de cobradores por confirmar"),
                    fix: Some((t("Open", "Abrir"), Fix::Payments)),
                });
            }
        }
        if let Some(r) = &self.updates.available {
            let v = &r.version;
            out.push(Reason {
                sev: 2,
                text: tr!("Version {v} is ready", "La versión {v} está lista"),
                fix: Some((t("See what's new", "Novedades"), Fix::Update)),
            });
        }
        out.sort_by_key(|r| std::cmp::Reverse(r.sev));
        (out.first().map_or(1, |r| r.sev), out)
    }

    fn ring_color(p: &Palette, sev: Severity) -> Hsla {
        match sev {
            3 => p.negative,
            2 => p.warning,
            1 => p.info,
            _ => p.faint,
        }
    }

    fn initials(&self) -> String {
        let (name, email) = self.who();
        super::initials_of(&name, if email.is_empty() { "?" } else { &email })
    }

    fn role_badge(role: Option<Role>) -> IconName {
        match role {
            Some(Role::Admin) | None => IconName::ShieldCheck,
            Some(Role::Collector) => IconName::BriefcaseBusiness,
            Some(Role::Viewer) => IconName::Eye,
        }
    }

    fn run_fix(&mut self, fix: Fix, window: &mut Window, cx: &mut Context<Self>) {
        self.account_open = false;
        match fix {
            Fix::Settings => self.navigate(Route::Settings, window, cx),
            Fix::Payments => self.navigate(Route::Payments, window, cx),
            Fix::Update => self.open_update(window, cx),
            Fix::Backup => self.backup_now(&super::BackupNow, window, cx),
        }
        cx.notify();
    }

    /// Collector payments waiting for an admin (none for collectors).
    pub(crate) fn waiting_to_confirm(&self) -> usize {
        if !self.can(Need::Manage) {
            return 0;
        }
        self.cloud_copy().map_or(0, |s| s.waiting().count())
    }

    pub(crate) fn unread_messages(&self) -> usize {
        self.messages().iter().filter(|m| m.read_at.is_none()).count()
    }

    /// The bell and the account icon, on the hero's glass (the light/dark
    /// switch is in the account panel, as on the web).
    pub(crate) fn account_cluster(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let h = p.hero();
        let unread = self.unread_messages();
        let bell = self.is_linked().then(|| {
            let tip =
                if unread > 0 { tr!("Messages, {unread} new", "Mensajes, {unread} nuevos") } else { t("Messages", "Mensajes").to_string() };
            h_flex()
                .id("bell-head")
                .flex_none()
                .h(px(36.))
                .min_w(px(36.))
                .px(px(10.))
                .gap(px(6.))
                .justify_center()
                .rounded_full()
                .bg(h.glass)
                .border_1()
                .border_color(h.glass_edge)
                .cursor_pointer()
                .hover(|d| d.bg(h.glass_hover))
                .relative()
                .child(icon(IconName::Bell, h.ink, 15.))
                // New messages: a blue badge on the bell's corner.
                .when(unread > 0, |d| d.child(div().absolute().top(px(-3.)).right(px(-3.)).child(Badge::new(unread, BadgeTone::Info))))
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                .on_click(cx.listener(|this, _, w, cx| this.open_messages(w, cx)))
        });
        let (sev, reasons) = self.app_status();
        let role = self.role();
        let why = if reasons.is_empty() {
            self.headline(sev).to_string()
        } else {
            reasons.iter().map(|r| r.text.as_str()).collect::<Vec<_>>().join(" · ")
        };
        let who = role.map(role_label).unwrap_or_default();
        let avatar = StatusAvatar::new("account", self.initials(), Self::ring_color(&p, sev), Self::role_badge(role))
            .photo(self.own_photo())
            .tooltip(format!("{who} · {why}"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.account_open = !this.account_open;
                cx.notify();
            }));
        let panel = self.account_open.then(|| self.account_panel(&p, sev, reasons, cx));
        h_flex()
            .flex_none()
            .gap(px(space::S))
            .children(bell)
            .child(div().relative().flex_none().child(avatar).children(panel))
            .into_any_element()
    }

    fn headline(&self, sev: Severity) -> &'static str {
        match sev {
            3 => t("Needs your attention", "Necesita tu atención"),
            2 => t("Something needs a look", "Algo necesita revisión"),
            1 => t("Everything is up to date", "Todo está al día"),
            _ => t("Checking…", "Comprobando…"),
        }
    }

    fn account_panel(&self, p: &Palette, sev: Severity, reasons: Vec<Reason>, cx: &mut Context<Self>) -> Overlay {
        let ring = Self::ring_color(p, sev);
        let role = self.role();
        let (name, email) = self.who();
        let item = |id: &'static str, glyph: IconName, title: &str, sub: Option<String>, f: MenuAction, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .gap(px(11.))
                .p(px(space::S))
                .rounded(px(RADIUS_CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(p.hover))
                .child(h_flex().flex_none().size(px(30.)).rounded(px(8.)).justify_center().bg(p.hover).child(icon(glyph, p.muted, 15.)))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(title.to_string()))
                        .children(sub.map(|s| div().typeset(text::CAPTION).text_color(p.muted).truncate().child(s))),
                )
                .on_click(cx.listener(move |this, _, w, cx| {
                    this.account_open = false;
                    f(this, w, cx);
                    cx.notify();
                }))
        };
        let status_rows: Vec<AnyElement> = if reasons.is_empty() {
            vec![
                div()
                    .typeset(text::CAPTION)
                    .text_color(p.muted)
                    .child(if sev == 0 { t("Connecting", "Conectando") } else { t("Synced", "Sincronizado") })
                    .into_any_element(),
            ]
        } else {
            reasons
                .into_iter()
                .enumerate()
                .map(|(i, r)| {
                    h_flex()
                        .gap(px(space::S))
                        .child(div().flex_none().size(px(6.)).rounded_full().bg(Self::ring_color(p, r.sev)))
                        .child(div().flex_1().min_w_0().typeset(text::CAPTION).text_color(p.muted).child(r.text))
                        .children(r.fix.map(|(label, fix)| {
                            Button::new(("acct-fix", i as u64))
                                .small()
                                .label(label)
                                .on_click(cx.listener(move |this, _, w, cx| this.run_fix(fix, w, cx)))
                        }))
                        .into_any_element()
                })
                .collect()
        };
        let mut businesses: Vec<AnyElement> = Vec::new();
        if let Some(c) = &self.cloud {
            let open = c.linked.clone();
            for (i, b) in c.businesses.iter().flatten().enumerate() {
                let id = b.business_id.clone();
                let on = open.as_deref() == Some(b.business_id.as_str());
                let letters: String =
                    b.name.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect();
                businesses.push(
                    h_flex()
                        .id(("acct-biz", i as u64))
                        .gap(px(11.))
                        .p(px(space::S))
                        .rounded(px(RADIUS_CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(p.hover))
                        .child(
                            h_flex()
                                .flex_none()
                                .size(px(30.))
                                .rounded(px(8.))
                                .justify_center()
                                .bg(p.hover)
                                .typeset(text::MICRO)
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.muted)
                                .child(letters),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().typeset(text::BODY_STRONG).text_color(p.text).truncate().child(b.name.clone()))
                                .child(div().typeset(text::CAPTION).text_color(p.muted).child(role_label(b.role))),
                        )
                        .children(on.then(|| icon(IconName::Check, p.accent_text, 15.)))
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.account_open = false;
                            if !on {
                                this.link_business(&id, w, cx);
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            if c.online {
                businesses.push(
                    item(
                        "acct-new-biz",
                        IconName::Plus,
                        t("New business", "Nuevo negocio"),
                        None,
                        Box::new(|this, w, cx| this.open_new_business(w, cx)),
                        cx,
                    )
                    .into_any_element(),
                );
            }
        }
        let backup_sub = match &self.settings().last_backup_at {
            Some(when) => tr!("Last backup {when}", "Última copia {when}"),
            None => t("Not backed up yet", "Sin copia de seguridad").to_string(),
        };
        let section = |label: Option<&str>| {
            v_flex().px(px(space::S)).py(px(6.)).border_t_1().border_color(p.border).children(
                label
                    .map(|l| div().px(px(space::S)).pt(px(4.)).pb(px(2.)).typeset(text::MICRO).text_color(p.faint).child(l.to_uppercase())),
            )
        };
        let close = cx.entity().downgrade();
        Overlay::new("account-panel", move |_, cx| {
            let _ = close.update(cx, |this, cx| {
                this.account_open = false;
                cx.notify();
            });
        })
        .align(OverlayAlign::End)
        .min_width(330.)
        .child(
            h_flex()
                .gap(px(space::M))
                .p(px(space::M))
                .child(StatusAvatar::new("account-big", self.initials(), ring, Self::role_badge(role)).photo(self.own_photo()))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        // The name, with the email under it; without a name, the email alone.
                        .child(div().typeset(text::BODY_STRONG).text_color(p.text).truncate().child(if name.is_empty() {
                            email.clone()
                        } else {
                            name.clone()
                        }))
                        .when(!name.is_empty(), |d| {
                            d.child(div().typeset(text::CAPTION).text_color(p.muted).truncate().child(email.clone()))
                        })
                        .child(
                            div()
                                .id("acct-edit-profile")
                                .mt(px(4.))
                                .typeset(text::CAPTION)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.text)
                                .cursor_pointer()
                                .hover(|d| d.underline())
                                .child(t("Edit profile", "Editar perfil"))
                                .on_click(cx.listener(|this, _, w, cx| {
                                    this.account_open = false;
                                    this.open_profile(w, cx);
                                    cx.notify();
                                })),
                        ),
                )
                .children(role.map(|r| pill(p, role_label(r), Tone::Accent))),
        )
        .child(
            v_flex()
                .mx(px(space::S))
                .mb(px(space::S))
                .p(px(space::M))
                .gap(px(6.))
                .rounded(px(10.))
                .bg(crate::ui::kit::interact::mix(p.surface, ring, 0.1))
                .border_1()
                .border_color(crate::ui::kit::interact::mix(p.border, ring, 0.35))
                .child(
                    h_flex()
                        .gap(px(space::S))
                        .child(div().size(px(9.)).rounded_full().bg(ring))
                        .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(self.headline(sev))),
                )
                .children(status_rows),
        )
        .child(section(Some(t("Business", "Negocio"))).children(businesses))
        .child(
            section(None)
                .child(item(
                    "acct-help",
                    IconName::BookOpen,
                    t("Help", "Ayuda"),
                    Some(t("How Acervus works, in plain words", "Cómo funciona Acervus, en palabras sencillas").to_string()),
                    Box::new(|this, w, cx| this.open_help(w, cx)),
                    cx,
                ))
                .child(item(
                    "acct-tour",
                    IconName::Compass,
                    t("Take the tour", "Hacer el recorrido"),
                    Some(t("A one-minute look at the essentials", "Un vistazo de un minuto a lo esencial").to_string()),
                    Box::new(|this, w, cx| this.start_tour(w, cx)),
                    cx,
                )),
        )
        .child(
            section(None).child(
                h_flex()
                    .gap(px(11.))
                    .p(px(space::S))
                    .child(h_flex().flex_none().size(px(30.)).rounded(px(8.)).justify_center().bg(p.hover).child(icon(
                        IconName::Palette,
                        p.muted,
                        15.,
                    )))
                    .child(div().flex_1().typeset(text::BODY_STRONG).text_color(p.text).child(t("Appearance", "Apariencia")))
                    .child({
                        let dark = self.settings().theme == ThemePref::Dark;
                        ThemePill::new(
                            "theme-pill",
                            dark,
                            if dark { t("Switch to light", "Cambiar a claro") } else { t("Switch to dark", "Cambiar a oscuro") },
                            cx.listener(move |this, _, w, cx| {
                                this.update_settings(|s| s.theme = if dark { ThemePref::Light } else { ThemePref::Dark }, w, cx)
                            }),
                        )
                    }),
            ),
        )
        .child(
            section(None)
                .when(self.can(Need::Manage), |d| {
                    d.child(item(
                        "acct-backup",
                        IconName::DatabaseBackup,
                        t("Back up database", "Copia de seguridad"),
                        Some(backup_sub),
                        Box::new(|this, w, cx| this.backup_now(&super::BackupNow, w, cx)),
                        cx,
                    ))
                })
                .child(item(
                    "acct-sync",
                    IconName::RefreshCw,
                    t("Sync options", "Opciones de sincronización"),
                    self.sync_look().map(|(_, label, _)| label),
                    Box::new(|this, w, cx| this.navigate(Route::Settings, w, cx)),
                    cx,
                ))
                .child(item(
                    "acct-settings",
                    IconName::Settings,
                    t("Settings", "Configuración"),
                    Some(t("Business, team, receipts, formats", "Negocio, equipo, recibos, formatos").to_string()),
                    Box::new(|this, w, cx| this.navigate(Route::Settings, w, cx)),
                    cx,
                )),
        )
        .child(
            h_flex()
                .id("acct-sign-out")
                .gap(px(11.))
                .px(px(space::L))
                .py(px(10.))
                .border_t_1()
                .border_color(p.border)
                .cursor_pointer()
                .hover(|d| d.bg(p.negative_tint))
                .text_color(p.negative)
                .typeset(text::BODY)
                .child(icon(IconName::LogOut, p.negative, 15.))
                .child(div().flex_1().child(t("Sign out", "Cerrar sesión")))
                .child(div().typeset(text::CAPTION).text_color(p.faint).child(format!(
                    "{} {}",
                    t("Version", "Versión"),
                    env!("CARGO_PKG_VERSION")
                )))
                .on_click(cx.listener(|this, _, w, cx| {
                    this.account_open = false;
                    this.sign_out(w, cx);
                })),
        )
    }
}
