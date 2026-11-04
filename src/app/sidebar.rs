use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Route, VzApp};
use crate::i18n::t;
use crate::tr;
use crate::ui::components::{Tone, icon, pill};
use crate::ui::kit::interact::{faded, hover_state, scoped};
use crate::ui::kit::{Button, ButtonKind, ButtonSize, ClickHandler, Dropdown, DropdownItem};
use crate::ui::theme::{Palette, RADIUS_CONTROL, SIDEBAR_COLLAPSED, SIDEBAR_WIDTH, Typeset as _, motion, palette, space, text};

fn nav() -> [(Route, IconName, &'static str, &'static str); 6] {
    [
        (Route::Overview, IconName::LayoutDashboard, t("Overview", "Resumen"), "Ctrl+1"),
        (Route::Clients, IconName::Users, t("Clients", "Clientes"), "Ctrl+2"),
        (Route::Loans, IconName::HandCoins, t("Loans", "Préstamos"), "Ctrl+3"),
        (Route::Payments, IconName::ArrowLeftRight, t("Payments", "Pagos"), "Ctrl+4"),
        (Route::Analytics, IconName::ChartLine, t("Portfolio Analytics", "Análisis"), "Ctrl+5"),
        (Route::Settings, IconName::Settings, t("Settings", "Configuración"), "Ctrl+6"),
    ]
}

/// The product mark: three ascending bars, the shape of a growing portfolio.
pub(crate) fn mark(p: &Palette) -> Div {
    let mut faded = p.accent;
    faded.a = 0.45;
    let mut mid = p.accent;
    mid.a = 0.72;
    h_flex()
        .flex_none()
        .size(px(24.))
        .rounded(px(7.))
        .bg(p.accent_tint)
        .items_end()
        .justify_center()
        .gap(px(2.))
        .pb(px(6.))
        .child(div().w(px(3.)).h(px(5.)).rounded(px(1.)).bg(faded))
        .child(div().w(px(3.)).h(px(8.)).rounded(px(1.)).bg(mid))
        .child(div().w(px(3.)).h(px(12.)).rounded(px(1.)).bg(p.accent))
}

/// One sidebar destination: icon, label and an optional badge (just the
/// icon when collapsed). The hover fill springs in like the kit's rows.
#[derive(IntoElement)]
struct NavItem {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    tip: SharedString,
    active: bool,
    collapsed: bool,
    badge: Option<AnyElement>,
    /// Times the bell rang so far (each new one swings the icon).
    ring: u32,
    on_click: ClickHandler,
}

impl NavItem {
    fn new(
        id: impl Into<ElementId>,
        icon: IconName,
        label: impl Into<SharedString>,
        tip: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            label: label.into(),
            tip: tip.into(),
            active: false,
            collapsed: false,
            badge: None,
            ring: 0,
            on_click: Rc::new(on_click),
        }
    }

    fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    fn badge(mut self, badge: Option<impl IntoElement>) -> Self {
        self.badge = badge.map(|b| b.into_any_element());
        self
    }

    fn ring(mut self, times: u32) -> Self {
        self.ring = times;
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let (hovered, on_hover) = hover_state(&self.id, window, cx);
        let (active, tip, f) = (self.active, self.tip, self.on_click);
        let ink = if active { p.text } else { p.muted };
        // Selected: a filled chip with the icon in the accent (a neutral
        // chip in dark mode, a tint of the accent in light).
        // The filled chip behind it is the sidebar's gliding highlight.
        let mark = p.accent_text;
        let has_badge = self.badge.is_some();
        let glyph = icon(self.icon, if active { mark } else { ink }, 16.);
        let glyph = if self.ring > 0 {
            let key = ElementId::NamedChild(std::sync::Arc::new(scoped(&self.id, "ring")), self.ring.to_string().into());
            glyph
                .with_animation(key, Animation::new(std::time::Duration::from_millis(800)), |g, t| {
                    g.transform(Transformation::rotate(radians(crate::ui::kit::motion::swing(t))))
                })
                .into_any_element()
        } else {
            glyph.into_any_element()
        };
        h_flex()
            .id(self.id.clone())
            .h(px(NAV_ITEM))
            .mx(px(10.))
            .px(px(10.))
            .gap(px(10.))
            .rounded(px(RADIUS_CONTROL))
            .cursor_pointer()
            .when(self.collapsed, |d| d.justify_center().px_0())
            .child(glyph)
            .when(!self.collapsed, |d| {
                d.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .typeset(if active { text::BODY_STRONG } else { text::BODY })
                        .text_color(ink)
                        .child(self.label),
                )
                .children(self.badge)
                .when(active && !has_badge, |d| d.child(div().flex_none().size(px(7.)).rounded_full().bg(p.accent)))
            })
            .on_hover(on_hover)
            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            .on_click(move |e, w, cx| f(e, w, cx))
            .with_spring(scoped(&self.id, "fill"), SpringAnimation::new(motion::CONTROL).to(hovered && !active), move |d, t| {
                d.bg(if active { gpui_kit::transparent_black() } else { faded(p.hover, t.0) })
            })
    }
}

impl VzApp {
    pub(crate) fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let collapsed = self.sidebar_collapsed(window);
        let section = self.route.section();
        let summary = self.summary_now();
        let overdue = summary.overdue_loans;
        let overdue_rises = self.motion.overdue.see(overdue);
        if let Some(status) = self.sync_status() {
            use crate::app::SyncStatus;
            self.note_sync(matches!(status, SyncStatus::Sending(_)), status == SyncStatus::Synced, cx);
        }

        // The business shown, with a menu to switch to another or start one.
        let switcher = self.cloud.as_ref().and_then(|c| {
            let list = c.businesses.as_ref()?;
            let open = c.linked.as_deref()?;
            let name = list.iter().find(|b| b.business_id == open)?.name.clone();
            let items = list.iter().map(|b| {
                let id = b.business_id.clone();
                DropdownItem::new(b.name.clone(), cx.listener(move |this, _, w, cx| this.link_business(&id, w, cx)))
                    .selected(b.business_id == open)
            });
            let new = DropdownItem::new(t("New business", "Nuevo negocio"), cx.listener(|this, _, w, cx| this.open_new_business(w, cx)))
                .icon(IconName::Plus)
                .disabled(!c.online);
            Some((
                name,
                Dropdown::new("business-switch")
                    .icon(IconName::ArrowRightLeft)
                    .kind(ButtonKind::Ghost)
                    .small()
                    .tooltip(t("Switch business", "Cambiar de negocio"))
                    .items(items)
                    .item(new),
            ))
        });
        let (title, menu) = match switcher {
            Some((name, menu)) => (SharedString::from(name), Some(menu)),
            None => (SharedString::from(crate::APP_NAME), None),
        };
        let items = nav();
        // Analytics shows the business's totals: admins only.
        let analytics = self.can(crate::sync::wire::Need::Manage);
        // The bell: messages in the business this PC follows.
        let unread = self.messages().iter().filter(|m| m.read_at.is_none()).count();
        let unread_rises = self.motion.unread.see(unread);
        let bell = self.is_linked().then(|| {
            NavItem::new(
                "messages",
                IconName::Bell,
                t("Messages", "Mensajes"),
                t("Messages", "Mensajes"),
                cx.listener(|this, _, w, cx| this.open_messages(w, cx)),
            )
            .collapsed(collapsed)
            .badge((unread > 0).then(|| popping(pill(&p, unread.to_string(), Tone::Accent), "unread-pop", unread_rises)))
            .ring(unread_rises)
        });
        let shown: Vec<_> = items.iter().filter(|(route, ..)| *route != Route::Analytics || analytics).collect();
        let at = shown.iter().position(|(route, ..)| *route == section);
        let nav = shown.into_iter().map(|(route, icon_name, label, keys)| {
            let route = *route;
            let badge = (route == Route::Loans && overdue > 0)
                .then(|| popping(pill(&p, overdue.to_string(), Tone::Negative), "overdue-pop", overdue_rises));
            NavItem::new(
                SharedString::from(format!("nav-{:?}", route)),
                *icon_name,
                label.to_string(),
                format!("{label}  {keys}"),
                cx.listener(move |this, _, window, cx| this.navigate(route, window, cx)),
            )
            .active(section == route)
            .collapsed(collapsed)
            .badge(badge)
        });

        let backup_label = match &self.settings().last_backup_at {
            Some(when) => tr!("Last backup {when}", "Última copia {when}"),
            None => t("Not backed up yet", "Sin copia de seguridad").to_string(),
        };
        let backup_tone = if self.settings().last_backup_at.is_some() { p.faint } else { p.warning };
        let toggle = |id: &'static str, icon_name: IconName, tip: &'static str| {
            Button::new(id)
                .ghost()
                .small()
                .icon(icon_name)
                .tooltip(tip)
                .on_click(cx.listener(|this, _, w, cx| this.toggle_sidebar(&super::ToggleSidebar, w, cx)))
        };

        v_flex()
            .flex_none()
            .h_full()
            .overflow_hidden()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.border)
            .child(
                h_flex()
                    .h(px(64.))
                    .px(px(if collapsed { 0. } else { 20. }))
                    .when(collapsed, |d| d.justify_center())
                    .gap(px(10.))
                    .child(match &self.logo {
                        Some(logo) => div()
                            .flex_none()
                            .size(px(32.))
                            .rounded(px(7.))
                            .overflow_hidden()
                            .child(img(logo.clone()).size_full().object_fit(ObjectFit::Contain))
                            .into_any_element(),
                        None => mark(&p).into_any_element(),
                    })
                    .when(!collapsed, |d| {
                        d.child(div().flex_1().min_w_0().line_clamp(2).typeset(text::HEADING).text_color(p.text).child(title))
                            .children(menu)
                            .child(toggle("collapse", IconName::PanelLeftClose, t("Collapse sidebar  Ctrl+B", "Ocultar menú  Ctrl+B")))
                    }),
            )
            .when(collapsed, |d| {
                d.child(h_flex().justify_center().pb(px(space::S)).child(toggle(
                    "expand",
                    IconName::PanelLeftOpen,
                    t("Expand sidebar  Ctrl+B", "Mostrar menú  Ctrl+B"),
                )))
            })
            .child(
                v_flex()
                    .relative()
                    .gap(px(NAV_GAP))
                    .pt(px(space::XS))
                    // The highlight glides to the chosen item.
                    .children(at.map(|i| {
                        let (fill, edge) = if p.dark { (p.neutral_tint, p.border) } else { (p.accent_tint, gpui_kit::transparent_black()) };
                        div()
                            .absolute()
                            .left(px(10.))
                            .right(px(10.))
                            .h(px(NAV_ITEM))
                            .rounded(px(RADIUS_CONTROL))
                            .bg(fill)
                            .border_1()
                            .border_color(edge)
                            .with_spring(
                                "nav-highlight",
                                SpringAnimation::new(motion::GLIDE).to(px(space::XS + i as f32 * (NAV_ITEM + NAV_GAP))),
                                |d, y| d.top(y),
                            )
                    }))
                    .children(nav),
            )
            .child(div().flex_1())
            .child(
                v_flex()
                    .gap(px(2.))
                    .pb(px(space::L))
                    .when_some(collapsed.then(|| self.sync_look()).flatten(), |d, (icon_name, label, _)| {
                        d.child(
                            NavItem::new(
                                "sync",
                                icon_name,
                                label.clone(),
                                tr!("Cloud sync: {label}", "Sincronización: {label}"),
                                cx.listener(|this, _, w, cx| this.navigate(Route::Settings, w, cx)),
                            )
                            .collapsed(true),
                        )
                    })
                    .children(bell)
                    .child(
                        NavItem::new(
                            "backup",
                            IconName::DatabaseBackup,
                            t("Back up database", "Copia de seguridad"),
                            t("Back up database  Ctrl+S", "Copia de seguridad  Ctrl+S"),
                            cx.listener(|this, _, w, cx| this.backup_now(&super::BackupNow, w, cx)),
                        )
                        .collapsed(collapsed),
                    )
                    .when(!collapsed, |d| {
                        d.child(
                            // A small card at the foot, like the cloud card in the redesign.
                            v_flex()
                                .mx(px(12.))
                                .mt(px(10.))
                                .p(px(space::M))
                                .rounded(px(12.))
                                .bg(crate::ui::kit::face(&p))
                                .border_1()
                                .border_color(p.border)
                                .shadow(crate::ui::theme::elevation(&p, crate::ui::theme::Elevation::Raised))
                                .gap(px(2.))
                                .typeset(text::CAPTION)
                                .text_color(p.faint)
                                // Signed in, the sync status takes the place of "stored on this PC".
                                .child(match self.sync_chip("sidebar-sync", cx) {
                                    Some(chip) => h_flex().pb(px(space::XS)).child(chip).into_any_element(),
                                    None => h_flex()
                                        .gap(px(6.))
                                        .child(icon(IconName::ShieldCheck, p.faint, 13.))
                                        .child(t("Private · stored on this PC", "Privado · guardado en esta PC"))
                                        .into_any_element(),
                                })
                                .child(div().text_color(backup_tone).child(backup_label))
                                .child(format!("{} {}", t("Version", "Versión"), env!("CARGO_PKG_VERSION")))
                                // Only when a newer version is out.
                                .children(self.updates.available.as_ref().map(|r| {
                                    let new = &r.version;
                                    div().pt(px(space::S)).child(
                                        Button::new("update-available")
                                            .primary()
                                            .size(ButtonSize::Mini)
                                            .wide()
                                            .label(t("Update available", "Actualización disponible"))
                                            .tooltip(tr!(
                                                "Version {new} is out: see what's new",
                                                "Salió la versión {new}: mira las novedades"
                                            ))
                                            .on_click(cx.listener(|this, _, w, cx| this.open_update(w, cx))),
                                    )
                                })),
                        )
                    }),
            )
            // Folding and unfolding glide instead of snapping.
            .with_spring(
                "sidebar-width",
                SpringAnimation::new(motion::GLIDE).to(px(if collapsed { SIDEBAR_COLLAPSED } else { SIDEBAR_WIDTH })),
                |d, w| d.w(w),
            )
            .into_any_element()
    }
}

/// A sidebar item's height, and the space between items.
const NAV_ITEM: f32 = 36.;
const NAV_GAP: f32 = 2.;

/// A count badge that pops each time its count goes up (`rises` so far).
fn popping(badge: Div, id: &'static str, rises: u32) -> AnyElement {
    let badge = badge.h(px(20.)).px(px(6.));
    if rises == 0 {
        return badge.into_any_element();
    }
    badge
        .with_animation((id, rises as u64), Animation::new(std::time::Duration::from_millis(420)), |d, t| {
            let k = crate::ui::kit::motion::pop(t);
            d.h(px(20. + 4. * k)).px(px(6. + 2. * k)).text_size(px(12. + 2. * k))
        })
        .into_any_element()
}
