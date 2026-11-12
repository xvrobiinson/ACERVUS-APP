//! "Filter by" (owner 2026-11-11): the Filter button beside a list page's
//! search, its panel of choices, and the chips of the filters that are on,
//! each with an × and "Clear all". The rules are `services::list_filters`;
//! the choices are kept in this PC's settings.

use std::collections::HashMap;

use chrono::NaiveDate;
use gpui_kit::assets::IconName;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::date_field::{DateChanged, DateField};
use crate::app::VzApp;
use crate::database::repositories::payments::PaidWith;
use crate::domain::loans::{Frequency, Method};
use crate::i18n::t;
use crate::services::list_filters::{CLIENT_STATUSES, ClientsBy, LOAN_STATUSES, LoansBy, PAID_PARTS, PaymentsBy, When};
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::icon;
use crate::ui::kit::Button;
use crate::ui::theme::{Typeset as _, palette, space, text};

/// Takes one filter off (a chip's ×).
type TakeOff = Box<dyn Fn(&mut VzApp, &mut Window, &mut Context<VzApp>)>;

/// The page a filter belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterPage {
    Payments,
    Loans,
    Clients,
}

/// One group of choices in the panel.
pub struct Group {
    pub key: &'static str,
    pub title: &'static str,
    pub options: Vec<(String, String)>,
    pub chosen: Vec<String>,
    pub single: bool,
}

/// A page's choices: its groups and, for Payments and Loans, a stretch of days.
pub struct Spec {
    pub groups: Vec<Group>,
    pub when: Option<(&'static str, When, Option<NaiveDate>, Option<NaiveDate>)>,
}

/// A stretch of days' name ("This month", "5 Jan to 5 Feb").
pub fn when_name(w: When, from: Option<NaiveDate>, to: Option<NaiveDate>, day: impl Fn(NaiveDate) -> String) -> String {
    match w {
        When::Today => t("Today", "Hoy").into(),
        When::Week => t("This week", "Esta semana").into(),
        When::Month => t("This month", "Este mes").into(),
        When::ThreeMonths => t("Last 3 months", "Últimos 3 meses").into(),
        When::Year => t("This year", "Este año").into(),
        When::All => t("All time", "Todo el tiempo").into(),
        When::Custom => match (from.map(&day), to.map(&day)) {
            (Some(a), Some(b)) => tr!("{a} to {b}", "{a} al {b}"),
            (Some(a), None) => tr!("From {a}", "Desde {a}"),
            (None, Some(b)) => tr!("Until {b}", "Hasta {b}"),
            (None, None) => t("Your own dates", "Tus fechas").into(),
        },
    }
}

fn loan_status_name(k: &str) -> &'static str {
    match k {
        "ok" => t("On time", "Al día"),
        "soon" => t("Due soon", "Vence pronto"),
        "late" => t("Late up to 30 days", "Atrasados hasta 30 días"),
        "later" => t("Late over 30 days", "Atrasados más de 30 días"),
        "paid" => t("Paid off", "Saldado"),
        "gone" => t("Defaulted", "Incobrable"),
        _ => t("Restructured", "Reestructurado"),
    }
}

fn client_status_name(k: &str) -> &'static str {
    match k {
        "late" => t("Has a late loan", "Tiene un préstamo atrasado"),
        "ontime" => t("All on time", "Todo al día"),
        _ => t("No open loans", "Sin préstamos abiertos"),
    }
}

fn part_name(k: &str) -> &'static str {
    match k {
        "fee" => t("Included a late fee", "Incluyó mora"),
        "interestOnly" => t("Interest only", "Solo interés"),
        _ => t("Paid capital", "Pagó capital"),
    }
}

fn keys<T: Copy>(all: &[T], key: impl Fn(T) -> &'static str, label: impl Fn(T) -> &'static str) -> Vec<(String, String)> {
    all.iter().map(|x| (key(*x).to_string(), label(*x).to_string())).collect()
}

impl VzApp {
    /// Who brought each payment in, by its number: the collector of a
    /// confirmed collector payment, else who recorded it (user ids; as
    /// Reports count it).
    pub(crate) fn payment_recorders(&self) -> HashMap<i64, String> {
        let mut out = HashMap::new();
        if let Some(snap) = self.cloud_copy() {
            // Who collected each confirmed payment, looked up once.
            let collected: HashMap<i64, &str> =
                snap.pending.iter().filter(|x| x.status == "confirmed").map(|x| (x.record_no, x.collected_by.as_str())).collect();
            for p in &snap.payments {
                let by = collected.get(&p.record_no).map_or_else(|| p.created_by.clone(), |c| c.to_string());
                out.insert(p.record_no, by);
            }
        }
        out
    }

    /// A person's name for the filters ("You" for the person here).
    fn person_name(&self, user: &str) -> String {
        match self.cloud_copy() {
            Some(s) if s.me.user_id == user => t("You", "Tú").to_string(),
            Some(s) if user != "none" => s.name_of(user).to_string(),
            _ => t("No collector", "Sin cobrador").to_string(),
        }
    }

    /// The collectors to filter by, and "No collector" (admins and managers
    /// of a business with collectors).
    fn collector_options(&self) -> Option<Vec<(String, String)>> {
        self.manages_collectors().then(|| {
            let mut list = self.takers();
            list.push(("none".into(), t("No collector", "Sin cobrador").into()));
            list
        })
    }

    pub(crate) fn filter_spec(&self, page: FilterPage) -> Spec {
        let s = self.settings();
        match page {
            FilterPage::Payments => {
                let f = &s.payments_filter;
                let mut groups = vec![Group {
                    key: "methods",
                    title: t("Payment method", "Forma de pago"),
                    options: keys(&PaidWith::ALL, PaidWith::key, PaidWith::label),
                    chosen: f.methods.clone(),
                    single: false,
                }];
                let people: Vec<(String, String)> = self
                    .cloud_copy()
                    .map(|c| c.people.iter().filter(|p| p.active).map(|p| (p.user_id.clone(), self.person_name(&p.user_id))).collect())
                    .unwrap_or_default();
                if self.can(Need::Books) && people.len() > 1 {
                    groups.push(Group {
                        key: "by",
                        title: t("Recorded by", "Registrado por"),
                        options: people,
                        chosen: f.by.clone(),
                        single: false,
                    });
                }
                groups.push(Group {
                    key: "parts",
                    title: t("What it paid", "Qué pagó"),
                    options: PAID_PARTS.iter().map(|k| (k.to_string(), part_name(k).to_string())).collect(),
                    chosen: f.parts.clone(),
                    single: false,
                });
                Spec { groups, when: Some((t("Period", "Período"), f.when, f.from, f.to)) }
            }
            FilterPage::Loans => {
                let f = &s.loans_filter;
                let mut groups = vec![Group {
                    key: "statuses",
                    title: t("Status", "Estado"),
                    options: LOAN_STATUSES.iter().map(|k| (k.to_string(), loan_status_name(k).to_string())).collect(),
                    chosen: f.statuses.clone(),
                    single: false,
                }];
                if let Some(options) = self.collector_options() {
                    groups.push(Group {
                        key: "collectors",
                        title: t("Collector", "Cobrador"),
                        options,
                        chosen: f.collectors.clone(),
                        single: false,
                    });
                }
                groups.push(Group {
                    key: "methods",
                    title: t("Loan type", "Tipo de préstamo"),
                    options: keys(&Method::ALL, Method::key, Method::short_label),
                    chosen: f.methods.clone(),
                    single: false,
                });
                groups.push(Group {
                    key: "frequencies",
                    title: t("How often", "Cada cuánto"),
                    options: keys(&Frequency::ALL, Frequency::key, Frequency::label),
                    chosen: f.frequencies.clone(),
                    single: false,
                });
                Spec { groups, when: Some((t("Lent on", "Prestado el"), f.when, f.from, f.to)) }
            }
            FilterPage::Clients => {
                let f = &s.clients_filter;
                let mut groups = vec![Group {
                    key: "statuses",
                    title: t("Status", "Estado"),
                    options: CLIENT_STATUSES.iter().map(|k| (k.to_string(), client_status_name(k).to_string())).collect(),
                    chosen: f.statuses.clone(),
                    single: false,
                }];
                if let Some(options) = self.collector_options() {
                    groups.push(Group {
                        key: "collectors",
                        title: t("Collector", "Cobrador"),
                        options,
                        chosen: f.collectors.clone(),
                        single: false,
                    });
                }
                groups.push(Group {
                    key: "archived",
                    title: t("Show", "Mostrar"),
                    options: vec![
                        ("active".into(), t("Active", "Activos").into()),
                        ("archived".into(), t("Archived", "Archivados").into()),
                    ],
                    chosen: vec![if f.archived { "archived" } else { "active" }.into()],
                    single: true,
                });
                Spec { groups, when: None }
            }
        }
    }

    /// Sets one group's choices.
    pub(crate) fn set_filter(&mut self, page: FilterPage, key: &'static str, v: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(
            |s| match (page, key) {
                (FilterPage::Payments, "methods") => s.payments_filter.methods = v,
                (FilterPage::Payments, "by") => s.payments_filter.by = v,
                (FilterPage::Payments, _) => s.payments_filter.parts = v,
                (FilterPage::Loans, "statuses") => s.loans_filter.statuses = v,
                (FilterPage::Loans, "collectors") => s.loans_filter.collectors = v,
                (FilterPage::Loans, "methods") => s.loans_filter.methods = v,
                (FilterPage::Loans, _) => s.loans_filter.frequencies = v,
                (FilterPage::Clients, "statuses") => s.clients_filter.statuses = v,
                (FilterPage::Clients, "collectors") => s.clients_filter.collectors = v,
                (FilterPage::Clients, _) => s.clients_filter.archived = v.first().is_some_and(|x| x == "archived"),
            },
            window,
            cx,
        );
    }

    pub(crate) fn set_filter_when(
        &mut self,
        page: FilterPage,
        w: When,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update_settings(
            |s| match page {
                FilterPage::Payments => (s.payments_filter.when, s.payments_filter.from, s.payments_filter.to) = (w, from, to),
                FilterPage::Loans => (s.loans_filter.when, s.loans_filter.from, s.loans_filter.to) = (w, from, to),
                FilterPage::Clients => {}
            },
            window,
            cx,
        );
    }

    /// "Clear all": every payment, loan or active client again.
    pub(crate) fn clear_filter(&mut self, page: FilterPage, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(
            |s| match page {
                FilterPage::Payments => s.payments_filter = PaymentsBy { when: When::All, ..Default::default() },
                FilterPage::Loans => s.loans_filter = LoansBy::default(),
                FilterPage::Clients => s.clients_filter = ClientsBy::default(),
            },
            window,
            cx,
        );
    }

    /// The filters that are on, each with how to take it off.
    fn filter_chips_of(&self, page: FilterPage) -> Vec<(String, TakeOff)> {
        let spec = self.filter_spec(page);
        let ds = self.settings().date_style;
        let mut out: Vec<(String, TakeOff)> = Vec::new();
        if let Some((title, w, from, to)) = spec.when.filter(|(_, w, _, _)| *w != When::All) {
            let name = when_name(w, from, to, |d| ds.format(d));
            let label = if page == FilterPage::Loans { format!("{title}: {name}") } else { name };
            out.push((label, Box::new(move |a, win, cx| a.set_filter_when(page, When::All, None, None, win, cx))));
        }
        for g in spec.groups {
            for v in &g.chosen {
                if g.single {
                    if v != "archived" {
                        continue;
                    }
                    out.push((
                        t("Archived", "Archivados").into(),
                        Box::new(move |a, win, cx| a.set_filter(page, g.key, vec!["active".into()], win, cx)),
                    ));
                    continue;
                }
                let label = g.options.iter().find(|(k, _)| k == v).map_or_else(|| self.person_name(v), |(_, l)| l.clone());
                let rest: Vec<String> = g.chosen.iter().filter(|x| *x != v).cloned().collect();
                let key = g.key;
                out.push((label, Box::new(move |a, win, cx| a.set_filter(page, key, rest.clone(), win, cx))));
            }
        }
        out
    }

    /// The Filter button, with how many filters are on.
    pub(crate) fn filter_button(&self, page: FilterPage, cx: &mut Context<Self>) -> Button {
        let p = palette(cx);
        let on = self.filter_chips_of(page).len();
        Button::new(("filter-btn", page as u64))
            .small()
            .icon(IconName::SlidersHorizontal)
            .label(t("Filter", "Filtrar"))
            .when(on > 0, |b| {
                b.suffix(
                    div()
                        .min_w(px(18.))
                        .h(px(18.))
                        .px(px(5.))
                        .rounded_full()
                        .bg(p.accent)
                        .text_color(p.on_accent)
                        .text_size(px(11.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(on.to_string()),
                )
            })
            .on_click(cx.listener(move |this, _, w, cx| this.open_filter(page, w, cx)))
    }

    /// The chips of the filters that are on, and "Clear all"; none when none.
    pub(crate) fn filter_chips(&self, page: FilterPage, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chips = self.filter_chips_of(page);
        if chips.is_empty() {
            return None;
        }
        let p = palette(cx);
        let many = chips.len() > 1;
        Some(
            h_flex()
                .flex_wrap()
                .gap(px(6.))
                .pb(px(space::M))
                .children(chips.into_iter().enumerate().map(|(i, (label, remove))| {
                    h_flex()
                        .h(px(28.))
                        .pl(px(12.))
                        .pr(px(4.))
                        .gap(px(4.))
                        .rounded_full()
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border)
                        .typeset(text::CAPTION)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text)
                        .child(label)
                        .child(
                            div()
                                .id(("filter-off", (page as u64) * 1000 + i as u64))
                                .size(px(20.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(p.hover))
                                .child(icon(IconName::X, p.muted, 12.))
                                .on_click(cx.listener(move |this, _, w, cx| remove(this, w, cx))),
                        )
                }))
                .when(many, |d| {
                    d.child(
                        div()
                            .id(("filter-clear", page as u64))
                            .px(px(6.))
                            .typeset(text::CAPTION)
                            .text_color(p.muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(p.text))
                            .child(t("Clear all", "Quitar todo"))
                            .on_click(cx.listener(move |this, _, w, cx| this.clear_filter(page, w, cx))),
                    )
                })
                .into_any_element(),
        )
    }

    pub(crate) fn open_filter(&mut self, page: FilterPage, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        // Its dates start on the page's (read here: the app is busy inside).
        let dates = self.filter_spec(page).when.map_or((None, None), |(_, _, a, b)| (a, b));
        let panel = cx.new(|cx| FilterPanel::new(app, page, dates, window, cx));
        let title = match page {
            FilterPage::Payments => t("Filter payments", "Filtrar pagos"),
            FilterPage::Loans => t("Filter loans", "Filtrar préstamos"),
            FilterPage::Clients => t("Filter clients", "Filtrar clientes"),
        };
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.entrance(gpui_kit::component::dialog::DialogEntrance::FadeSlide).title(title).w(px(560.)).child(panel.clone())
        });
    }
}

/// The panel: each group's choices as buttons, the days' stretch with your
/// own dates, then Clear all and Done. It shows the app's filters as they
/// change.
pub struct FilterPanel {
    app: Entity<VzApp>,
    page: FilterPage,
    from: Entity<DateField>,
    to: Entity<DateField>,
    _subs: Vec<Subscription>,
}

impl FilterPanel {
    fn new(
        app: Entity<VzApp>,
        page: FilterPage,
        (from, to): (Option<NaiveDate>, Option<NaiveDate>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let today = crate::utils::dates::today();
        let from = super::date_field(from.unwrap_or(today), window, cx);
        let to = super::date_field(to.unwrap_or(today), window, cx);
        let mut subs = vec![cx.observe(&app, |_, _, cx| cx.notify())];
        for field in [&from, &to] {
            subs.push(cx.subscribe_in(field, window, |this, _, _: &DateChanged, window, cx| {
                let (a, b) = (super::picked_date(&this.from, cx), super::picked_date(&this.to, cx));
                let page = this.page;
                this.app.update(cx, |app, cx| app.set_filter_when(page, When::Custom, a, b, window, cx));
            }));
        }
        Self { app, page, from, to, _subs: subs }
    }
}

impl Render for FilterPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let page = self.page;
        let spec = self.app.read(cx).filter_spec(page);
        let ds = self.app.read(cx).settings().date_style;
        let title = |s: &str| div().typeset(text::LABEL).text_color(p.text).child(s.to_string());
        let mut body = v_flex().gap(px(space::L));
        for (gi, g) in spec.groups.into_iter().enumerate() {
            let buttons = g.options.iter().enumerate().map(|(oi, (value, label))| {
                let on = g.chosen.contains(value);
                let next: Vec<String> = if g.single {
                    vec![value.clone()]
                } else if on {
                    g.chosen.iter().filter(|x| *x != value).cloned().collect()
                } else {
                    g.chosen.iter().cloned().chain([value.clone()]).collect()
                };
                let key = g.key;
                Button::new(("filter-opt", (gi * 100 + oi) as u64)).small().label(label.clone()).selected(on).on_click(cx.listener(
                    move |this, _, w, cx| {
                        let v = next.clone();
                        this.app.update(cx, |app, cx| app.set_filter(page, key, v, w, cx));
                    },
                ))
            });
            body = body.child(v_flex().gap(px(space::S)).child(title(g.title)).child(h_flex().flex_wrap().gap(px(6.)).children(buttons)));
        }
        if let Some((name, w, from, to)) = spec.when {
            let choices = When::ALL.iter().enumerate().map(|(i, x)| {
                let x = *x;
                Button::new(("filter-when", i as u64))
                    .small()
                    .label(if x == When::Custom {
                        t("Your own dates", "Tus fechas").to_string()
                    } else {
                        when_name(x, None, None, |d| ds.format(d))
                    })
                    .selected(w == x)
                    .on_click(cx.listener(move |this, _, win, cx| {
                        let (a, b) = if x == When::Custom {
                            (super::picked_date(&this.from, cx), super::picked_date(&this.to, cx))
                        } else {
                            (from, to)
                        };
                        this.app.update(cx, |app, cx| app.set_filter_when(page, x, a, b, win, cx));
                    }))
            });
            body =
                body.child(v_flex().gap(px(space::S)).child(title(name)).child(h_flex().flex_wrap().gap(px(6.)).children(choices)).when(
                    w == When::Custom,
                    |d| {
                        d.child(
                            h_flex()
                                .gap(px(space::M))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .gap(px(4.))
                                        .child(div().typeset(text::CAPTION).text_color(p.muted).child(t("From", "Desde")))
                                        .child(self.from.clone()),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .gap(px(4.))
                                        .child(div().typeset(text::CAPTION).text_color(p.muted).child(t("To", "Hasta")))
                                        .child(self.to.clone()),
                                ),
                        )
                    },
                ));
        }
        body.child(
            h_flex()
                .justify_end()
                .gap(px(space::S))
                .pt(px(space::S))
                .child(Button::new("filter-clear-all").ghost().label(t("Clear all", "Quitar todo")).on_click(cx.listener(
                    move |this, _, w, cx| {
                        this.app.update(cx, |app, cx| app.clear_filter(page, w, cx));
                    },
                )))
                .child(Button::new("filter-done").primary().label(t("Done", "Listo")).on_click(|_, w, cx| w.close_dialog(cx))),
        )
    }
}

#[cfg(test)]
mod tests {
    // The kit's glob import also brings GPUI's `test` macro; keep the
    // built-in one for what `#[gpui::test]` expands to.
    use ::gpui;
    use core::prelude::v1::test;

    use super::*;
    use crate::services::Store;
    use crate::ui::forms::test_app;

    #[gpui::test]
    fn a_filter_is_kept_on_this_pc_until_it_is_cleared(cx: &mut TestAppContext) {
        let (app, cx) = test_app(cx, Store::in_memory());
        cx.update(|w, cx| app.update(cx, |a, cx| a.set_filter(FilterPage::Loans, "methods", vec!["flat".into()], w, cx)));
        cx.update(|w, cx| app.update(cx, |a, cx| a.set_filter_when(FilterPage::Loans, When::Year, None, None, w, cx)));
        let methods = cx.update(|_, cx| app.read(cx).settings().loans_filter.methods.clone());
        assert_eq!(methods, vec!["flat".to_string()]);
        // Saved in this PC's settings, read back as the page's filter.
        let saved = cx.update(|_, cx| app.read(cx).store.snapshot().map(|s| s.settings.loans_filter));
        assert_eq!(saved.ok().map(|f| (f.methods, f.when)), Some((vec!["flat".to_string()], When::Year)));
        // A chip for each, then the panel opens over the page.
        assert_eq!(cx.update(|_, cx| app.read(cx).filter_chips_of(FilterPage::Loans).len()), 2);
        cx.update(|w, cx| app.update(cx, |a, cx| a.open_filter(FilterPage::Loans, w, cx)));
        cx.run_until_parked();
        cx.update(|w, cx| app.update(cx, |a, cx| a.clear_filter(FilterPage::Loans, w, cx)));
        assert_eq!(cx.update(|_, cx| app.read(cx).filter_chips_of(FilterPage::Loans).len()), 0);
        // Payments start on the last three months, shown as a chip.
        assert_eq!(cx.update(|_, cx| app.read(cx).filter_chips_of(FilterPage::Payments).len()), 1);
    }
}
