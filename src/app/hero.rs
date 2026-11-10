//! The clean look (the owner's pick from a drawing, 2026-11-09; the web's
//! src/clean.css): a plain grey page (near-black in dark) with no picture.
//! A bar on top holds the mark and the name, the sections as pills (the open
//! one black, white in dark), search in the middle, the bell and the
//! account; it stays put while the page scrolls. The page starts with a
//! greeting, the business (it opens the switcher) and the date, then the
//! Overview's figures, then the screen's cards straight on the page.

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Route, VzApp};
use crate::i18n::t;
use crate::sync::wire::Need;
use crate::ui::components::sel;
use crate::ui::kit::{Badge, BadgeTone, Logo, tour_mark};
use crate::ui::theme::{self, Typeset as _, palette, text};

/// The top bar's height.
const BAR: f32 = 72.;
/// The widest the content gets; wider windows center it.
const MAX: f32 = 1440.;

/// The space at each side of the bar and the page.
fn side_pad(viewport: f32) -> f32 {
    theme::Width::gutter(viewport).max((viewport - MAX) / 2.)
}

/// The width a screen's content gets.
pub(crate) fn content_width(viewport: f32) -> f32 {
    viewport - 2. * side_pad(viewport)
}

/// The section nav: Settings is in the account menu; Portfolio is for admins.
fn hero_nav(admin: bool) -> Vec<(Route, IconName, &'static str, &'static str)> {
    [
        (Route::Overview, IconName::LayoutDashboard, t("Overview", "Resumen"), "Ctrl+1"),
        (Route::Clients, IconName::Users, t("Clients", "Clientes"), "Ctrl+2"),
        (Route::Loans, IconName::HandCoins, t("Loans", "Préstamos"), "Ctrl+3"),
        (Route::Payments, IconName::ArrowLeftRight, t("Payments", "Pagos"), "Ctrl+4"),
        (Route::Calendar, IconName::CalendarDays, t("Calendar", "Calendario"), ""),
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
    /// A screen in the clean look: the bar on top, then on the page the
    /// greeting, `figures` (the Overview's) and `content`, with `banner`
    /// (data that failed to load) above the content.
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
        let pad = side_pad(f32::from(window.viewport_size().width));

        // The sections: Loans counts the loans overdue (a red badge that hops
        // when the count rises), Payments the collector payments to confirm.
        let section = self.route.section();
        let overdue = self.summary_now().overdue_loans;
        let overdue_rises = self.motion.overdue.see(overdue);
        let waiting = self.waiting_to_confirm();
        // One pill: its word, and its count after it. The open one is filled.
        let tab = |id: SharedString, label: &'static str, tip: SharedString, on: bool, badge: Option<(Badge, u32)>| {
            h_flex()
                .id(id)
                .flex_none()
                .h(px(32.))
                .px(px(14.))
                .gap(px(6.))
                .rounded_full()
                .cursor_pointer()
                .typeset(text::LABEL)
                .map(|d| if on { d.bg(h.ink).text_color(h.mark_ink).font_weight(FontWeight::SEMIBOLD) } else { d.text_color(h.mute) })
                .when(!on, |d| d.hover(|d| d.bg(h.glass_hover).text_color(h.ink)))
                .child(label)
                .map(|d| match badge {
                    Some((b, rises)) => d.child(div().child(b).with_animation(
                        SharedString::from(format!("badge-hop-{label}-{rises}")),
                        Animation::new(Duration::from_millis(420)),
                        |d, t| d.mt(px(-3. * crate::ui::kit::motion::pop(t))),
                    )),
                    None => d,
                })
                .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
        };
        let mut nav: Vec<AnyElement> = Vec::new();
        for (route, _, label, keys) in hero_nav(self.can(Need::Manage)) {
            let badge = match route {
                Route::Loans if overdue > 0 => Some((Badge::new(overdue, BadgeTone::Alert), overdue_rises)),
                Route::Payments if waiting > 0 => Some((Badge::new(waiting, BadgeTone::Waiting), 0)),
                _ => None,
            };
            let tip = SharedString::from(format!("{label}  {keys}"));
            let mark = match route {
                Route::Clients => Some("nav-clients"),
                Route::Payments => Some("nav-payments"),
                Route::Analytics => Some("nav-portfolio"),
                _ => None,
            };
            nav.push(
                tab(SharedString::from(format!("hero-nav-{route:?}")), label, tip, section == route, badge)
                    .on_click(cx.listener(move |this, _, w, cx| this.navigate(route, w, cx)))
                    .when_some(mark, |d, k| d.relative().child(tour_mark(k, &self.tour_targets)))
                    .into_any_element(),
            );
        }
        // Reports (admins and viewers): its window, not a screen.
        if self.can_report() {
            let label = t("Reports", "Reportes");
            nav.push(
                tab("hero-nav-reports".into(), label, label.into(), false, None)
                    .on_click(cx.listener(|this, _, w, cx| this.open_reports(w, cx)))
                    .into_any_element(),
            );
        }
        let pills =
            h_flex().flex_none().p(px(4.)).gap(px(2.)).rounded_full().bg(h.glass).border_1().border_color(h.glass_edge).children(nav);
        let brand = h_flex()
            .id("hero-brand")
            .flex_none()
            .gap(px(10.))
            .cursor_pointer()
            .child(if p.dark { Logo::new(30.).edge(h.glass_edge) } else { Logo::new(30.) })
            .child(
                h_flex()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(h.ink)
                    .child("Acervus")
                    .child(div().text_color(p.faint).child(".")),
            )
            .on_click(cx.listener(|this, _, w, cx| this.navigate(Route::Overview, w, cx)));
        let search = self.hero_search(window, cx);
        let cluster = self.account_cluster(window, cx);
        let bar = h_flex()
            .flex_none()
            .h(px(BAR))
            .px(px(pad))
            .gap(px(14.))
            .bg(p.ground)
            .child(brand)
            .child(pills)
            .child(div().flex_1().min_w(px(4.)))
            .child(h_flex().flex_none().relative().gap(px(8.)).child(search).child(cluster).child(tour_mark("right", &self.tour_targets)));

        // The greeting, the business and the date.
        let (title, switchable, menu) = self.brand_switcher(&p, cx);
        let letters: String = title.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect();
        let mark = match &self.logo {
            Some(logo) => div()
                .flex_none()
                .size(px(20.))
                .rounded(px(6.))
                .overflow_hidden()
                .child(img(logo.clone()).size_full().object_fit(ObjectFit::Contain)),
            None => div()
                .flex_none()
                .size(px(20.))
                .rounded(px(6.))
                .bg(h.mark)
                .text_color(h.mark_ink)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(9.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(letters),
        };
        let business = h_flex()
            .id("hero-biz")
            .relative()
            .gap(px(8.))
            .mx(px(-6.))
            .px(px(6.))
            .py(px(2.))
            .rounded(px(8.))
            .typeset(text::LABEL)
            .text_color(h.ink)
            .child(mark)
            .child(
                div()
                    .relative()
                    .whitespace_nowrap()
                    .child(sel(title))
                    // Admins on the Overview: a small ⇄ under the name says
                    // it switches (2026-11-10, the owner's pick).
                    .when(switchable && self.route == Route::Overview && self.can(crate::sync::wire::Need::Manage), |d| {
                        d.child(
                            h_flex().absolute().top(relative(1.)).left_0().right_0().justify_center().child(crate::ui::components::icon(
                                IconName::ArrowLeftRight,
                                h.mute,
                                12.,
                            )),
                        )
                    }),
            )
            .when(switchable, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(h.glass_hover))
                    .when(self.brand_open, |d| d.bg(h.glass_hover))
                    .tooltip(|w, cx| Tooltip::new(t("Switch business", "Cambiar de negocio")).build(w, cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.brand_open = !this.brand_open;
                        cx.notify();
                    }))
            })
            .children(menu);
        // Arrange, on the Overview: at the right of the greeting.
        let arrange = (self.route == Route::Overview && !self.customizing).then(|| self.arrange_button(h.ink, cx));
        let identity = h_flex()
            .px(px(pad))
            .pt(px(10.))
            .pb(px(18.))
            .items_end()
            .gap(px(14.))
            .child(
                v_flex()
                    .min_w_0()
                    .gap(px(4.))
                    .child(div().typeset(text::HERO_NAME).font_weight(FontWeight::SEMIBOLD).text_color(h.ink).child(greeting()))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .typeset(text::LABEL)
                            .child(business)
                            .child(div().text_color(h.mute).child("·"))
                            .child(div().font_weight(FontWeight::NORMAL).text_color(h.mute).child(today_line())),
                    ),
            )
            .child(div().flex_1())
            .children(arrange);

        // A new screen fades in while rising into place (at launch, once the
        // splash starts to open).
        let covering = self.motion.covering;
        let screen = crate::ui::kit::motion::rise(content, SharedString::from(format!("screen-{:?}", self.route)), Duration::ZERO);
        let body = v_flex()
            .px(px(pad))
            .gap(px(14.))
            .children(banner.map(|b| div().rounded(px(12.)).overflow_hidden().child(b)))
            .when(!covering, |d| d.child(screen));
        let page = div().id(SharedString::from(format!("scroll-{section:?}"))).flex_1().min_h_0().overflow_y_scroll().child(
            v_flex()
                .w_full()
                .pb(px(48.))
                .child(identity)
                .children(figures.map(|f| {
                    div().px(px(pad)).pb(px(14.)).child(div().relative().child(f).child(tour_mark("figures", &self.tour_targets)))
                }))
                .child(body),
        );

        v_flex()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(p.ground)
            .child(bar)
            .child(page)
            // The "+" button, on the Overview (approved 2026-11-05).
            .when(self.route == Route::Overview, |d| {
                let mark = tour_mark("add", &self.tour_targets);
                d.children(self.add_button(cx).map(|f| div().absolute().right(px(28.)).bottom(px(28.)).child(f).child(mark)))
            })
            .into_any_element()
    }
}
