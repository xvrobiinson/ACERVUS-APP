//! The hero look (the web's, approved 2026-11-05/06, on every screen since
//! 2026-11-06): no sidebar; a header over the owner's fog picture (light) or
//! black with light streaks (dark) holds the section nav, search, the bell
//! and the account, then today's date, a greeting and the business (its name
//! or logo opens the switcher). It stays put while the page scrolls under
//! it: the Overview's figures on glass cards, then a rounded panel with the
//! screen. The picture is one image drawn twice, in the header and behind
//! the page, at the same size so it lines up.

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{FocusSearch, Route, VzApp};
use crate::i18n::t;
use crate::sync::wire::Need;
use crate::ui::components::{icon, sel};
use crate::ui::kit::{Badge, BadgeTone};
use crate::ui::theme::{self, Elevation, Typeset as _, palette, text};

/// The header's height: the nav row and the date, greeting and business.
const HEADER: f32 = 190.;
/// The picture's height; it fades into the page at its bottom.
const BACKDROP: f32 = 640.;
/// The widest the content gets; wider windows center it.
const MAX: f32 = 1440.;
/// The panel's inner padding.
const PANEL_PAD: f32 = 22.;

/// The space at each side of the header and the panel.
fn side_pad(viewport: f32) -> f32 {
    theme::Width::gutter(viewport).max((viewport - MAX) / 2.)
}

/// The width a screen's content gets inside the panel.
pub(crate) fn content_width(viewport: f32) -> f32 {
    viewport - 2. * side_pad(viewport) - 2. * PANEL_PAD
}

/// The section nav: Settings is in the account menu; Portfolio is for admins.
fn hero_nav(admin: bool) -> Vec<(Route, IconName, &'static str, &'static str)> {
    [
        (Route::Overview, IconName::LayoutDashboard, t("Overview", "Resumen"), "Ctrl+1"),
        (Route::Clients, IconName::Users, t("Clients", "Clientes"), "Ctrl+2"),
        (Route::Loans, IconName::HandCoins, t("Loans", "Préstamos"), "Ctrl+3"),
        (Route::Payments, IconName::ArrowLeftRight, t("Payments", "Pagos"), "Ctrl+4"),
        (Route::Analytics, IconName::ChartLine, t("Portfolio", "Portafolio"), "Ctrl+5"),
    ]
    .into_iter()
    .filter(|(r, ..)| *r != Route::Analytics || admin)
    .collect()
}

/// Today as the hero shows it: "Friday, 6 November 2026".
fn today_line() -> String {
    use chrono::Datelike as _;
    let d = chrono::Local::now().date_naive();
    let day = [
        t("Monday", "lunes"),
        t("Tuesday", "martes"),
        t("Wednesday", "miércoles"),
        t("Thursday", "jueves"),
        t("Friday", "viernes"),
        t("Saturday", "sábado"),
        t("Sunday", "domingo"),
    ][d.weekday().num_days_from_monday() as usize];
    let month = [
        t("January", "enero"),
        t("February", "febrero"),
        t("March", "marzo"),
        t("April", "abril"),
        t("May", "mayo"),
        t("June", "junio"),
        t("July", "julio"),
        t("August", "agosto"),
        t("September", "septiembre"),
        t("October", "octubre"),
        t("November", "noviembre"),
        t("December", "diciembre"),
    ][d.month0() as usize];
    let s = if crate::i18n::is_es() {
        format!("{day}, {} de {month} de {}", d.day(), d.year())
    } else {
        format!("{day}, {} {month} {}", d.day(), d.year())
    };
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// The greeting for the time of day.
pub(crate) fn greeting() -> &'static str {
    use chrono::Timelike as _;
    match chrono::Local::now().hour() {
        5..=11 => t("Good morning", "Buenos días"),
        12..=17 => t("Good afternoon", "Buenas tardes"),
        18..=21 => t("Good evening", "Buenas noches"),
        _ => t("Good night", "Buenas noches"),
    }
}

impl VzApp {
    /// A screen in the hero look: `figures` (the Overview's) on the picture,
    /// `content` on the panel below, `banner` (data that failed to load) at
    /// the panel's top.
    pub(crate) fn render_hero(
        &mut self,
        figures: Option<AnyElement>,
        content: AnyElement,
        banner: Option<impl IntoElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = palette(cx);
        let h = p.hero();
        if let Some(status) = self.sync_status() {
            use crate::app::SyncStatus;
            self.note_sync(matches!(status, SyncStatus::Sending(_)), status == SyncStatus::Synced, cx);
        }
        let picture = crate::ui::pictures::hero(p.dark);
        let pad = side_pad(f32::from(window.viewport_size().width));
        let backdrop =
            |top: f32| img(picture.clone()).absolute().top(px(top)).left_0().w_full().h(px(BACKDROP)).object_fit(ObjectFit::Cover);
        let mut clear = p.ground;
        clear.a = 0.;
        let fade = |top: f32| {
            div().absolute().left_0().w_full().top(px(top)).h(px(BACKDROP)).bg(linear_gradient(
                180.,
                linear_color_stop(clear, 0.82),
                linear_color_stop(p.ground, 1.),
            ))
        };

        // The nav row: Loans counts the loans overdue (a red badge that hops
        // when the count rises), Payments the collector payments to confirm.
        let section = self.route.section();
        let overdue = self.summary_now().overdue_loans;
        let overdue_rises = self.motion.overdue.see(overdue);
        let waiting = self.waiting_to_confirm();
        // One tab: an icon and a word; the open one is underlined.
        let tab = |id: SharedString, glyph: IconName, label: &'static str, tip: SharedString, on: bool, badge: Option<(Badge, u32)>| {
            let color = if on { h.ink } else { h.mute };
            h_flex()
                .id(id)
                .relative()
                .h(px(60.))
                .px(px(10.))
                .gap(px(7.))
                .cursor_pointer()
                .typeset(text::LABEL)
                .text_color(color)
                .hover(|d| d.text_color(h.ink))
                .map(|d| match badge {
                    // The badge on the icon's top right corner; it hops when
                    // its count rises.
                    Some((b, rises)) => {
                        let hop = div().child(b).with_animation(
                            SharedString::from(format!("badge-hop-{label}-{rises}")),
                            Animation::new(Duration::from_millis(420)),
                            |d, t| d.mt(px(-3. * crate::ui::kit::motion::pop(t))),
                        );
                        d.child(
                            div()
                                .relative()
                                .mr(px(5.))
                                .child(icon(glyph, color, 15.))
                                .child(div().absolute().top(px(-9.)).left(px(9.)).child(hop)),
                        )
                        .child(label)
                    }
                    None => d.child(icon(glyph, color, 15.)).child(label),
                })
                .when(on, |d| d.child(div().absolute().left(px(10.)).right(px(10.)).bottom(px(14.)).h(px(1.5)).rounded(px(1.)).bg(h.ink)))
                .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
        };
        // A thin line before Portfolio and Reports, which go together.
        let divider = || div().flex_none().mx(px(6.)).w(px(1.)).h(px(18.)).bg(h.rule).into_any_element();
        let mut nav: Vec<AnyElement> = Vec::new();
        for (route, icon_name, label, keys) in hero_nav(self.can(Need::Manage)) {
            if route == Route::Analytics {
                nav.push(divider());
            }
            let badge = match route {
                Route::Loans if overdue > 0 => Some((Badge::new(overdue, BadgeTone::Alert), overdue_rises)),
                Route::Payments if waiting > 0 => Some((Badge::new(waiting, BadgeTone::Waiting), 0)),
                _ => None,
            };
            let tip = SharedString::from(format!("{label}  {keys}"));
            nav.push(
                tab(SharedString::from(format!("hero-nav-{route:?}")), icon_name, label, tip, section == route, badge)
                    .on_click(cx.listener(move |this, _, w, cx| this.navigate(route, w, cx)))
                    .into_any_element(),
            );
        }
        // Reports (admins and viewers): its window, not a screen.
        if self.can_report() {
            if !self.can(Need::Manage) {
                nav.push(divider());
            }
            let label = t("Reports", "Reportes");
            nav.push(
                tab("hero-nav-reports".into(), IconName::ReceiptText, label, label.into(), false, None)
                    .on_click(cx.listener(|this, _, w, cx| this.open_reports(w, cx)))
                    .into_any_element(),
            );
        }
        let search = h_flex()
            .id("hero-search")
            .h(px(36.))
            .w(px(260.))
            .pl(px(15.))
            .pr(px(4.))
            .gap(px(8.))
            .rounded_full()
            .bg(h.glass)
            .border_1()
            .border_color(h.glass_edge)
            .cursor_pointer()
            .hover(|d| d.bg(h.glass_hover))
            .typeset(text::CAPTION)
            .text_color(h.mute)
            .child(div().flex_1().min_w_0().truncate().child(t("Search clients & loans…", "Buscar clientes y préstamos…")))
            .child(div().size(px(28.)).rounded_full().flex().items_center().justify_center().bg(h.glass).child(icon(
                IconName::Search,
                h.ink,
                14.,
            )))
            .on_click(|_, w, cx| w.dispatch_action(Box::new(FocusSearch), cx));
        let cluster = self.account_cluster(window, cx);
        let bar = h_flex()
            .h(px(60.))
            .gap(px(4.))
            .border_b_1()
            .border_color(h.rule)
            .children(nav)
            .child(div().flex_1())
            .child(h_flex().gap(px(10.)).child(search).child(cluster));

        // The date, a greeting and the business.
        let (title, switchable, menu) = self.brand_switcher(&p, cx);
        let letters: String = title.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect();
        let mark = match &self.logo {
            Some(logo) => div()
                .flex_none()
                .size(px(42.))
                .rounded(px(12.))
                .overflow_hidden()
                .child(img(logo.clone()).size_full().object_fit(ObjectFit::Contain)),
            None => div()
                .flex_none()
                .size(px(42.))
                .rounded(px(12.))
                .bg(h.mark)
                .text_color(h.mark_ink)
                .flex()
                .items_center()
                .justify_center()
                .typeset(text::LABEL)
                .font_weight(FontWeight::SEMIBOLD)
                .child(letters),
        };
        let business = h_flex()
            .id("hero-biz")
            .relative()
            .gap(px(12.))
            .mx(px(-8.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(14.))
            .text_color(h.ink)
            .child(mark)
            .child(div().typeset(text::HERO_NAME).whitespace_nowrap().child(sel(title)))
            .when(switchable, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(h.glass))
                    .when(self.brand_open, |d| d.bg(h.glass))
                    .tooltip(|w, cx| Tooltip::new(t("Switch business", "Cambiar de negocio")).build(w, cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.brand_open = !this.brand_open;
                        cx.notify();
                    }))
            })
            .children(menu);
        // Arrange, on the Overview: the hero's top right, over the figures.
        let arrange = (self.route == Route::Overview && !self.customizing).then(|| self.arrange_button(h.mute, cx));
        let identity = v_flex()
            .pt(px(16.))
            .gap(px(3.))
            .child(div().typeset(text::CAPTION).text_color(h.mute).child(today_line()))
            .child(div().typeset(text::BODY).text_color(h.mute).child(greeting()))
            .child(h_flex().mt(px(6.)).items_center().child(business).child(div().flex_1()).children(arrange));

        let header = div()
            .relative()
            .flex_none()
            .w_full()
            .h(px(HEADER))
            .overflow_hidden()
            .child(backdrop(0.))
            .child(v_flex().relative().px(px(pad)).child(bar).child(identity));

        // A new screen fades in while rising into place (at launch, once the
        // splash starts to open).
        let covering = self.motion.covering;
        let screen = crate::ui::kit::motion::rise(content, SharedString::from(format!("screen-{:?}", self.route)), Duration::ZERO);
        let panel = v_flex()
            .relative()
            .mx(px(pad))
            .mt(px(-40.))
            .min_h(px(320.))
            .p(px(PANEL_PAD))
            .gap(px(14.))
            .rounded(px(20.))
            .bg(p.surface)
            .when(p.dark, |d| d.border_1().border_color(p.border))
            .shadow(theme::elevation(&p, Elevation::Floating))
            .children(banner.map(|b| div().rounded(px(12.)).overflow_hidden().child(b)))
            .when(!covering, |d| d.child(screen));
        let page = div().id(SharedString::from(format!("scroll-{section:?}"))).flex_1().min_h_0().overflow_y_scroll().child(
            div()
                .relative()
                .w_full()
                .pb(px(48.))
                .child(backdrop(-HEADER))
                .child(fade(-HEADER))
                .children(figures.map(|f| div().relative().px(px(pad)).pt(px(2.)).child(f)))
                .child(div().h(px(56.)))
                .child(panel),
        );

        v_flex()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(p.ground)
            .child(header)
            .child(page)
            // The "+" button, on the Overview (approved 2026-11-05).
            .when(self.route == Route::Overview, |d| {
                d.children(self.add_button(cx).map(|f| div().absolute().right(px(28.)).bottom(px(28.)).child(f)))
            })
            .into_any_element()
    }
}
