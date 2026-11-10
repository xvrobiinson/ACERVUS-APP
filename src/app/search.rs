//! Search from any screen (2026-11-10, usability item 10; the web's
//! `SearchBox` in src/ui/hero.tsx): a round magnifying glass beside the bell
//! that grows into a search box (or Ctrl+F). It finds clients by name, phone
//! or ID number and loans by number or client, five of each, and opens the
//! one picked. Escape, or leaving it empty, folds it back.

use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::{Route, VzApp};
use crate::domain::ledger::owed_at_payment;
use crate::i18n::t;
use crate::tr;
use crate::ui::components::icon;
use crate::ui::lists::matches;
use crate::ui::theme::{Elevation, Typeset as _, elevation, palette, space, text};

/// One result.
struct Hit {
    route: Route,
    title: String,
    sub: String,
    letters: String,
    client: bool,
}

/// How wide the box grows, and its results.
const BOX_W: f32 = 320.;
const RESULTS_W: f32 = 380.;
/// Results of each kind.
const EACH: usize = 5;

impl VzApp {
    /// Opens the box and puts the cursor in it (the button, Ctrl+F).
    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        self.global_search.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// Folds the box back into its button and forgets what was typed.
    pub(crate) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = false;
        self.global_search.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    fn search_hits(&self, query: &str) -> Vec<Hit> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let money = self.settings().money();
        let today = self.today;
        let clients = self
            .snap
            .clients
            .iter()
            .filter(|c| !c.is_archived())
            .filter(|c| {
                let digits: String = q.chars().filter(char::is_ascii_digit).collect();
                let phone: String = c.phone.as_deref().unwrap_or_default().chars().filter(char::is_ascii_digit).collect();
                matches(q, &[&c.full_name, c.id_number.as_deref().unwrap_or_default()]) || (digits.len() >= 3 && phone.contains(&digits))
            })
            .take(EACH)
            .map(|c| {
                let n = self.snap.loans_for_client(c.id).count();
                Hit {
                    route: Route::Client(c.id),
                    title: c.full_name.clone(),
                    sub: match n {
                        0 => c.phone.clone().unwrap_or_else(|| t("No loans", "Sin préstamos").to_string()),
                        1 => t("1 loan", "1 préstamo").to_string(),
                        n => tr!("{n} loans", "{n} préstamos"),
                    },
                    letters: c.full_name.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect(),
                    client: true,
                }
            });
        let number = q.trim_start_matches('#');
        let loans = self
            .snap
            .book
            .loans
            .iter()
            .filter(|l| number == l.id.to_string() || matches(q, &[self.snap.client_name(l.client_id)]))
            .take(EACH)
            .map(|l| {
                let (id, name) = (l.id, self.snap.client_name(l.client_id).to_string());
                let owed = match l.plan_state(today) {
                    Some(s) => s.payoff(),
                    None => owed_at_payment(&l.facts, &l.payments, today).payoff(),
                };
                Hit {
                    route: Route::Loan(id),
                    title: tr!("Loan #{id} · {name}", "Préstamo #{id} · {name}"),
                    sub: if owed > 0 {
                        let o = money.format(owed);
                        tr!("{o} to pay it off", "{o} para saldarlo")
                    } else {
                        t("Paid back", "Pagado").to_string()
                    },
                    letters: "#".into(),
                    client: false,
                }
            });
        clients.chain(loans).collect()
    }

    /// The button, or the open box with its results under it.
    pub(crate) fn hero_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let h = p.hero();
        if !self.search_open {
            return div()
                .id("hero-search")
                .flex_none()
                .size(px(40.))
                .rounded_full()
                .bg(h.glass)
                .border_1()
                .border_color(h.glass_edge)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(h.glass_hover))
                .child(icon(IconName::Search, h.ink, 17.))
                .tooltip(|w, cx| Tooltip::new(t("Search clients & loans  Ctrl+F", "Buscar clientes y préstamos  Ctrl+F")).build(w, cx))
                .on_click(cx.listener(|this, _, w, cx| this.open_search(w, cx)))
                .into_any_element();
        }
        let query = self.global_search.read(cx).value().to_string();
        let hits = self.search_hits(&query);
        let results = (!query.trim().is_empty()).then(|| {
            let group = |client: bool| {
                let rows: Vec<_> = hits.iter().filter(|x| x.client == client).collect();
                (!rows.is_empty()).then(|| {
                    v_flex()
                        .gap(px(2.))
                        .child(
                            div()
                                .px(px(space::M))
                                .pt(px(space::S))
                                .pb(px(4.))
                                .typeset(text::CAPTION)
                                .text_color(p.faint)
                                .child(if client { t("Clients", "Clientes") } else { t("Loans", "Préstamos") }),
                        )
                        .children(rows.into_iter().map(|x| {
                            let route = x.route;
                            h_flex()
                                .id(SharedString::from(format!("hit-{route:?}")))
                                .gap(px(space::M))
                                .px(px(space::M))
                                .py(px(6.))
                                .rounded(px(10.))
                                .cursor_pointer()
                                .hover(|d| d.bg(p.hover))
                                .child(
                                    div()
                                        .flex_none()
                                        .size(px(30.))
                                        .rounded_full()
                                        .bg(p.neutral_tint)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .typeset(text::CAPTION)
                                        .text_color(p.muted)
                                        .child(x.letters.clone()),
                                )
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .child(div().truncate().typeset(text::BODY_STRONG).text_color(p.text).child(x.title.clone()))
                                        .child(div().truncate().typeset(text::CAPTION).text_color(p.muted).child(x.sub.clone())),
                                )
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.close_search(w, cx);
                                    this.navigate(route, w, cx);
                                }))
                        }))
                })
            };
            let body: AnyElement = if hits.is_empty() {
                let q = query.trim().to_string();
                div()
                    .p(px(space::M))
                    .typeset(text::BODY)
                    .text_color(p.muted)
                    .child(tr!(
                        "Nothing matches \"{q}\". Try a name, a phone number or a loan number.",
                        "Nada coincide con \"{q}\". Prueba un nombre, un teléfono o un número de préstamo."
                    ))
                    .into_any_element()
            } else {
                v_flex().children(group(true)).children(group(false)).into_any_element()
            };
            div()
                .absolute()
                .top(px(48.))
                .right_0()
                .w(px(RESULTS_W))
                .p(px(6.))
                .rounded(px(14.))
                .bg(p.surface)
                .border_1()
                .border_color(p.border)
                .shadow(elevation(&p, Elevation::Floating))
                .occlude()
                .child(body)
        });
        div()
            .id("hero-search-open")
            .relative()
            .flex_none()
            .w(px(BOX_W))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| match e.keystroke.key.as_str() {
                "escape" => {
                    this.close_search(w, cx);
                    cx.stop_propagation();
                }
                "enter" => {
                    let q = this.global_search.read(cx).value().to_string();
                    if let Some(route) = this.search_hits(&q).first().map(|x| x.route) {
                        this.close_search(w, cx);
                        this.navigate(route, w, cx);
                    }
                }
                _ => {}
            }))
            .child(Input::new(&self.global_search).prefix(icon(IconName::Search, p.faint, 15.)).with_animation(
                "search-grow",
                Animation::new(std::time::Duration::from_millis(260)),
                |d, t| d.w(px(40. + (BOX_W - 40.) * crate::ui::kit::motion::ease_out(t))),
            ))
            .children(results)
            .into_any_element()
    }
}
