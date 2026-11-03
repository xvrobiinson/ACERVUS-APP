//! Collectors on the desktop: who collects from each client, the payments
//! collectors recorded that wait for an admin's confirmation, and the bell.
//! All of it comes from the cloud's copy of the business
//! (`sync::wire::Snapshot`); a business without collectors shows none of it.

use gpui_kit::assets::IconName;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ncolumn;
use crate::app::{Route, VzApp};
use crate::i18n::{count, t};
use crate::sync::change::Change;
use crate::sync::wire::{Message, Need, Pending, Role, Snapshot, View};
use crate::tr;
use crate::ui::components::{Tone, avatar, empty_state, icon, pill, sel, table_row, tone_colors};
use crate::ui::forms::reason::ReasonKind;
use crate::ui::kit::{Button, Card, Dropdown, DropdownItem};
use crate::ui::theme::{Palette, RADIUS_CONTROL, Typeset as _, Width, palette, space, tabular, text};

/// Which clients the Clients list shows, by collector.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CollectorFilter {
    #[default]
    All,
    /// Clients nobody collects from yet.
    Nobody,
    Person(String),
}

/// A waiting payment with this PC's loan and client numbers.
struct Row {
    pending: Pending,
    loan: i64,
    client: i64,
}

fn rows(snap: &Snapshot, pick: impl Fn(&Pending) -> bool) -> Vec<Row> {
    snap.pending
        .iter()
        .filter(|p| pick(p))
        .filter_map(|p| {
            let loan = snap.loans.iter().find(|l| l.id == p.loan_id)?.record_no;
            let client = snap.clients.iter().find(|c| c.id == p.client_id)?.record_no;
            Some(Row { pending: p.clone(), loan, client })
        })
        .collect()
}

impl VzApp {
    /// What this person may see: a collector's switches, or everything.
    pub(crate) fn view(&self) -> View {
        self.cloud_copy().and_then(|s| s.me.view).unwrap_or_default()
    }

    /// Whether the business has collectors (then clients show who collects).
    pub(crate) fn has_collectors(&self) -> bool {
        self.cloud_copy().is_some_and(Snapshot::has_collectors)
    }

    /// Admins see and change who collects from each client.
    pub(crate) fn manages_collectors(&self) -> bool {
        self.has_collectors() && self.can(Need::Manage)
    }

    /// A collector's or admin's name as the lists show it.
    pub(crate) fn collector_label(&self, user: Option<&str>) -> String {
        let nobody = || t("No collector", "Sin cobrador").to_string();
        let (Some(u), Some(s)) = (user, self.cloud_copy()) else { return nobody() };
        if u == s.me.user_id {
            return t("You", "Tú").to_string();
        }
        match s.name_of(u) {
            "" => nobody(),
            name => name.to_string(),
        }
    }

    /// The people who can be given clients, the person here first.
    fn takers(&self) -> Vec<(String, String)> {
        let Some(s) = self.cloud_copy() else { return Vec::new() };
        let mut list: Vec<(String, String)> = s
            .people
            .iter()
            .filter(|x| x.active && matches!(x.role, Some(Role::Admin | Role::Collector)))
            .map(|x| {
                let label = if x.user_id == s.me.user_id {
                    t("You (admin)", "Tú (administrador)").to_string()
                } else if x.role == Some(Role::Admin) {
                    format!("{} ({})", x.name, t("admin", "administrador"))
                } else {
                    x.name.clone()
                };
                (x.user_id.clone(), label)
            })
            .collect();
        list.sort_by_key(|(u, _)| u != &s.me.user_id);
        list
    }

    /// Who collects from a client, as a menu that changes it.
    pub(crate) fn collector_menu(&self, id: impl Into<ElementId>, client: i64, cx: &mut Context<Self>) -> AnyElement {
        let current = self.cloud_copy().and_then(|s| s.collector_of(client)).map(str::to_string);
        let mut items: Vec<DropdownItem> = self
            .takers()
            .into_iter()
            .map(|(user, name)| {
                let selected = current.as_deref() == Some(user.as_str());
                DropdownItem::new(name, cx.listener(move |this, _, w, cx| this.assign_clients(vec![client], Some(user.clone()), w, cx)))
                    .selected(selected)
            })
            .collect();
        items.push(
            DropdownItem::new(
                t("No collector", "Sin cobrador"),
                cx.listener(move |this, _, w, cx| this.assign_clients(vec![client], None, w, cx)),
            )
            .selected(current.is_none()),
        );
        Dropdown::new(id).small().label(self.collector_label(current.as_deref())).items(items).into_any_element()
    }

    /// The Clients list's filter by collector.
    pub(crate) fn collector_filter_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let label = match &self.collector_filter {
            CollectorFilter::All => t("Every collector", "Todos los cobradores").to_string(),
            CollectorFilter::Nobody => t("No collector", "Sin cobrador").to_string(),
            CollectorFilter::Person(u) => self.collector_label(Some(u)),
        };
        let mut choices = vec![(CollectorFilter::All, t("Every collector", "Todos los cobradores").to_string())];
        for (user, _) in self.takers() {
            let name = self.collector_label(Some(&user));
            choices.push((CollectorFilter::Person(user), name));
        }
        choices.push((CollectorFilter::Nobody, t("No collector", "Sin cobrador").to_string()));
        let items: Vec<DropdownItem> = choices
            .into_iter()
            .map(|(f, name)| {
                let selected = self.collector_filter == f;
                DropdownItem::new(
                    name,
                    cx.listener(move |this, _, _, cx| {
                        this.collector_filter = f.clone();
                        cx.notify();
                    }),
                )
                .selected(selected)
            })
            .collect();
        Dropdown::new("collector-filter").label(label).items(items).into_any_element()
    }

    /// Whether a client passes the Clients list's collector filter.
    pub(crate) fn passes_collector_filter(&self, client: i64) -> bool {
        let who = self.cloud_copy().and_then(|s| s.collector_of(client));
        match &self.collector_filter {
            CollectorFilter::All => true,
            CollectorFilter::Nobody => who.is_none(),
            CollectorFilter::Person(u) => who == Some(u.as_str()),
        }
    }

    pub(crate) fn assign_clients(&mut self, clients: Vec<i64>, collector: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let who = self.collector_label(collector.as_deref());
        match self.save(Change::AssignClients { clients, collector }, window, cx) {
            Ok(_) => self.notify_success(tr!("Now collected by {who}.", "Ahora lo cobra {who}."), window, cx),
            Err(e) => {
                let msg = self.error_message(&e);
                self.notify_error(msg, window, cx);
            }
        }
        cx.notify();
    }

    fn confirm_pending(&mut self, ids: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let n = ids.len();
        for id in ids {
            if let Err(e) = self.save(Change::ConfirmPending { pending: id }, window, cx) {
                let msg = self.error_message(&e);
                self.notify_error(msg, window, cx);
                return;
            }
        }
        self.notify_success(
            if n == 1 {
                t("Payment confirmed. It counts from the day it was collected.", "Pago confirmado. Cuenta desde el día en que se cobró.")
                    .to_string()
            } else {
                tr!("{n} payments confirmed.", "{n} pagos confirmados.")
            },
            window,
            cx,
        );
        cx.notify();
    }

    /// The cloud id of a waiting payment, by its receipt number.
    pub(crate) fn waiting_pending_id(&self, number: i64) -> Option<String> {
        self.cloud_copy()?.waiting().find(|p| p.record_no == number).map(|p| p.id.clone())
    }

    /// One payment that waits (or was decided), as a row.
    fn pending_row(&self, p: &Palette, r: &Row, decide: bool, compact: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let money = self.settings().money();
        let ds = self.settings().date_style;
        let x = &r.pending;
        let name = self.snap.client_name(r.client).to_string();
        let (loan, collector) = (r.loan, x.collector_name.clone());
        let mut sub = tr!("Loan #{loan} · collected by {collector}", "Préstamo #{loan} · cobrado por {collector}");
        if x.status == "rejected" {
            sub = format!("{sub} · “{}”", x.reason);
        }
        if compact {
            sub = format!("{} · {sub}", ds.format(x.paid_on.0));
        }
        let id = x.id.clone();
        let number = x.record_no;
        let status = match x.status.as_str() {
            "confirmed" => pill(p, t("Confirmed", "Confirmado"), Tone::Positive),
            "rejected" => pill(p, t("Rejected", "Rechazado"), Tone::Negative),
            _ => pill(p, t("Waiting for confirmation", "Esperando confirmación"), Tone::Warning),
        };
        let waiting = x.status == "waiting";
        table_row(p, ("pending", number as u64), false)
            .font_features(tabular())
            .when(!compact, |d| d.child(div().w(px(120.)).flex_none().child(sel(ds.format(x.paid_on.0)))))
            .child(
                h_flex().flex_1().min_w_0().gap(px(space::M)).child(avatar(p, &name, r.client, 28.)).child(
                    v_flex()
                        .min_w_0()
                        .child(div().truncate().child(sel(name)))
                        .child(div().typeset(text::CAPTION).text_color(p.muted).truncate().child(sel(sub))),
                ),
            )
            .child(ncolumn(130.).whitespace_nowrap().child(sel(money.format(x.amount))))
            .child(
                h_flex()
                    .flex_none()
                    .justify_end()
                    .gap(px(space::XS))
                    .when(!decide || !waiting, |d| d.child(status))
                    .when(waiting, |d| {
                        let rid = id.clone();
                        d.child(
                            Button::new(("pend-receipt", number as u64))
                                .ghost()
                                .small()
                                .icon(IconName::Printer)
                                .tooltip(t("Receipt (pending confirmation)", "Recibo (pendiente de confirmación)"))
                                .on_click(cx.listener(move |this, _, w, cx| this.open_pending_receipt(&rid, w, cx))),
                        )
                    })
                    .when(decide && waiting, |d| {
                        d.child(
                            Button::new(("pend-reject", number as u64)).ghost().small().label(t("Reject", "Rechazar")).on_click(
                                cx.listener(move |this, _, w, cx| this.open_reason_form(ReasonKind::RejectPending(number), w, cx)),
                            ),
                        )
                        .child(
                            Button::new(("pend-confirm", number as u64))
                                .primary()
                                .small()
                                .label(t("Confirm", "Confirmar"))
                                .on_click(cx.listener(move |this, _, w, cx| this.confirm_pending(vec![id.clone()], w, cx))),
                        )
                    }),
            )
    }

    /// Payments: what collectors collected and waits for an admin (admins
    /// and viewers), or a collector's own recent collections.
    pub(crate) fn confirm_card(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let snap = self.cloud_copy()?;
        let p = palette(cx);
        let compact = Width::of(self.content_width(window)) == Width::Compact;
        let money = self.settings().money();
        if snap.me.role == Role::Collector {
            let mut mine = rows(snap, |_| true);
            mine.sort_by_key(|r| (r.pending.status != "waiting", std::cmp::Reverse(r.pending.created_at)));
            if mine.is_empty() {
                return None;
            }
            let list: Vec<_> = mine.iter().take(20).map(|r| self.pending_row(&p, r, false, compact, cx)).collect();
            return Some(
                Card::new()
                    .overflow_hidden()
                    .title(t("My collected payments", "Mis cobros"))
                    .child(caption(
                        &p,
                        t(
                            "They count once the office confirms the money reached them; until then the loan's balance doesn't change.",
                            "Cuentan cuando la oficina confirme que el dinero le llegó; mientras tanto el saldo del préstamo no cambia.",
                        ),
                    ))
                    .child(v_flex().children(list))
                    .into_any_element(),
            );
        }
        let mut waiting = rows(snap, |x| x.status == "waiting");
        if waiting.is_empty() {
            return None;
        }
        waiting.sort_by_key(|r| r.pending.created_at);
        let decide = self.can(Need::Manage) && self.cloud.as_ref().is_some_and(|c| c.online);
        let mut people: Vec<String> = Vec::new();
        for r in &waiting {
            if !people.contains(&r.pending.collected_by) {
                people.push(r.pending.collected_by.clone());
            }
        }
        let total = money.format(waiting.iter().map(|r| r.pending.amount).sum());
        let n = count(waiting.len(), "payment", "payments", "pago", "pagos");
        let mut body = v_flex().child(caption(
            &p,
            t(
                "A collector's payment doesn't count until you confirm the money reached you: the balance, overdue and totals stay as they were. Once confirmed it counts from the day it was collected, so the client isn't charged late fees for the wait. Rejected, it is removed and the collector is told why.",
                "El pago de un cobrador no cuenta hasta que confirmes que el dinero te llegó: el saldo, el atraso y los totales quedan igual. Al confirmarlo cuenta desde el día en que se cobró, así que al cliente no se le cobra mora por la espera. Si lo rechazas, se quita y el cobrador sabe por qué.",
            ),
        ));
        for (gi, who) in people.iter().enumerate() {
            let group: Vec<&Row> = waiting.iter().filter(|r| &r.pending.collected_by == who).collect();
            let name = group[0].pending.collector_name.clone();
            let held = money.format(group.iter().map(|r| r.pending.amount).sum());
            let ids: Vec<String> = group.iter().map(|r| r.pending.id.clone()).collect();
            let many = ids.len();
            body = body.child(
                h_flex()
                    .px(px(space::XL))
                    .pt(px(space::L))
                    .pb(px(space::S))
                    .gap(px(space::M))
                    .child(div().typeset(text::HEADING).child(sel(name)))
                    .child(div().text_color(p.muted).font_features(tabular()).child(sel(tr!("holds {held}", "tiene {held}"))))
                    .child(div().flex_1())
                    .when(decide && many > 1, |d| {
                        d.child(
                            Button::new(("pend-all", gi as u64))
                                .primary()
                                .small()
                                .label(tr!("Confirm all {many}", "Confirmar los {many}"))
                                .on_click(cx.listener(move |this, _, w, cx| this.confirm_pending(ids.clone(), w, cx))),
                        )
                    }),
            );
            for r in group {
                body = body.child(self.pending_row(&p, r, decide, compact, cx));
            }
        }
        Some(
            Card::new()
                .overflow_hidden()
                .title(tr!("To confirm · {n}, {total}", "Por confirmar · {n}, {total}"))
                .child(body)
                .into_any_element(),
        )
    }

    /// The loan page: its collected payments still waiting for an admin.
    pub(crate) fn pending_on_loan(&self, loan: i64, compact: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let snap = self.cloud_copy()?;
        let list = rows(snap, |x| x.status == "waiting");
        let list: Vec<Row> = list.into_iter().filter(|r| r.loan == loan).collect();
        if list.is_empty() {
            return None;
        }
        let p = palette(cx);
        let decide = self.can(Need::Manage) && self.cloud.as_ref().is_some_and(|c| c.online);
        let items: Vec<_> = list.iter().map(|r| self.pending_row(&p, r, decide, compact, cx)).collect();
        Some(
            v_flex()
                .child(caption(
                    &p,
                    t(
                        "Collected, waiting for confirmation: not counted in the balance yet.",
                        "Cobrados, esperando confirmación: aún no cuentan en el saldo.",
                    ),
                ))
                .children(items)
                .into_any_element(),
        )
    }

    /// Overview: payments that wait for someone, with the way to them.
    pub(crate) fn pending_banner(&self, p: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let snap = self.cloud_copy()?;
        let waiting: Vec<&Pending> = snap.waiting().collect();
        if waiting.is_empty() {
            return None;
        }
        let money = self.settings().money();
        let sum = money.format(waiting.iter().map(|x| x.amount).sum());
        let n = waiting.len();
        let collector = snap.me.role == Role::Collector;
        let line = if collector {
            tr!(
                "{n} of your payments, {sum}, wait for the office to confirm them.",
                "{n} de tus cobros, {sum}, esperan que la oficina los confirme."
            )
        } else {
            tr!(
                "{n} collected payment(s), {sum}, wait for you to confirm the money reached you.",
                "{n} pago(s) cobrado(s), {sum}, esperan que confirmes que el dinero te llegó."
            )
        };
        let (bg, ink) = tone_colors(p, Tone::Warning);
        Some(
            h_flex()
                .gap(px(space::M))
                .px(px(space::L))
                .py(px(space::M))
                .rounded(px(RADIUS_CONTROL))
                .bg(bg)
                .child(icon(IconName::Wallet, ink, 16.))
                .child(div().flex_1().min_w_0().text_color(ink).font_features(tabular()).child(sel(line)))
                .child(
                    Button::new("pending-review")
                        .small()
                        .label(if collector { t("See them", "Verlos") } else { t("Review", "Revisar") })
                        .on_click(cx.listener(|this, _, w, cx| this.navigate(Route::Payments, w, cx))),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn open_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let fresh: Vec<String> = self.messages().iter().filter(|m| m.read_at.is_none()).map(|m| m.id.clone()).collect();
        let view = cx.new(|_| Bell { app, fresh });
        self.read_messages();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .entrance(gpui_kit::component::dialog::DialogEntrance::FadeSlide)
                .title(t("Messages", "Mensajes"))
                .w(px(560.))
                .child(view.clone())
        });
    }
}

/// Stands in for payment lists a collector's switches hide.
pub(crate) fn history_hidden(p: &Palette) -> AnyElement {
    empty_state(
        p,
        IconName::Receipt,
        t("Payment history is hidden", "El historial de pagos está oculto"),
        t("An admin chose not to show it to your account.", "Un administrador decidió no mostrarlo en tu cuenta."),
        None,
    )
    .into_any_element()
}

fn caption(p: &Palette, s: &str) -> Div {
    div().px(px(space::XL)).py(px(space::S)).typeset(text::CAPTION).text_color(p.muted).child(s.to_string())
}

/// A message's words, in this PC's language.
pub(crate) fn message_text(m: &Message, app: &VzApp) -> String {
    let money = app.settings().money();
    let ds = app.settings().date_style;
    let who = match m.client_names.len() {
        0..=3 => m.client_names.join(", "),
        n => {
            let more = n - 3;
            let first = m.client_names[..3].join(", ");
            tr!("{first} and {more} more", "{first} y {more} más")
        }
    };
    let amount = m.amount.map(|a| money.format(a)).unwrap_or_default();
    let due = m.due_on.map(|d| ds.format(d.0)).unwrap_or_default();
    let from = &m.from_name;
    let reason = m.reason.clone().unwrap_or_default();
    match m.kind.as_str() {
        "due_unassigned" => tr!("{who} pays {amount} on {due} and has no collector.", "{who} paga {amount} el {due} y no tiene cobrador."),
        "assigned" => {
            let n = m.client_names.len();
            if n == 1 {
                tr!("You were given a client: {who}.", "Te asignaron un cliente: {who}.")
            } else {
                tr!("You were given {n} clients: {who}.", "Te asignaron {n} clientes: {who}.")
            }
        }
        "unassigned" => tr!("No longer your clients: {who}.", "Ya no son tus clientes: {who}."),
        "collected" => {
            tr!(
                "{from} collected {amount} from {who}. It counts once you confirm it.",
                "{from} cobró {amount} a {who}. Cuenta cuando lo confirmes."
            )
        }
        "confirmed" => tr!("Your {amount} from {who} was confirmed.", "Se confirmó tu cobro de {amount} a {who}."),
        "rejected" => tr!("Your {amount} from {who} was rejected: “{reason}”", "Se rechazó tu cobro de {amount} a {who}: «{reason}»"),
        _ => String::new(),
    }
}

/// The messages dialog.
pub struct Bell {
    app: WeakEntity<VzApp>,
    /// The ones that were new when it opened stay marked while it's open.
    fresh: Vec<String>,
}

impl Render for Bell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let Some(app) = self.app.upgrade() else { return div().into_any_element() };
        let a = app.read(cx);
        let manage = a.can(Need::Manage);
        let ds = a.settings().date_style;
        let waiting: Vec<String> = a.cloud_copy().map(|s| s.waiting().map(|x| x.id.clone()).collect()).unwrap_or_default();
        let messages = a.messages().to_vec();
        if messages.is_empty() {
            return div().py(px(space::L)).text_color(p.muted).child(t("No messages.", "No hay mensajes.")).into_any_element();
        }
        let items: Vec<AnyElement> = messages
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let words = message_text(m, a);
                let when = chrono::DateTime::from_timestamp_millis(m.created_at)
                    .map(|d| {
                        let d = d.with_timezone(&chrono::Local);
                        format!("{} · {}", ds.format(d.date_naive()), d.format("%H:%M"))
                    })
                    .unwrap_or_default();
                let (tone, glyph) = match m.kind.as_str() {
                    "due_unassigned" => (Tone::Warning, IconName::CalendarClock),
                    "collected" => (Tone::Accent, IconName::Wallet),
                    "confirmed" => (Tone::Positive, IconName::Check),
                    "rejected" => (Tone::Negative, IconName::CircleX),
                    _ => (Tone::Accent, IconName::Users),
                };
                let (bg, ink) = tone_colors(&p, tone);
                let action: Option<(&'static str, Route)> = match m.kind.as_str() {
                    "due_unassigned" if manage => Some((t("Assign a collector", "Asignar cobrador"), Route::Clients)),
                    "collected" if manage && m.pending_id.as_ref().is_some_and(|id| waiting.contains(id)) => {
                        Some((t("Review and confirm", "Revisar y confirmar"), Route::Payments))
                    }
                    _ => None,
                };
                let weak = self.app.clone();
                h_flex()
                    .items_start()
                    .gap(px(space::M))
                    .p(px(space::M))
                    .rounded(px(RADIUS_CONTROL))
                    .when(self.fresh.contains(&m.id), |d| d.bg(p.accent_tint))
                    .child(
                        h_flex()
                            .flex_none()
                            .size(px(28.))
                            .rounded(px(8.))
                            .bg(bg)
                            .justify_center()
                            .items_center()
                            .child(icon(glyph, ink, 14.)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(div().typeset(text::BODY).child(sel(words)))
                            .child(div().typeset(text::CAPTION).text_color(p.muted).child(when))
                            .when_some(action, |d, (label, route)| {
                                let nobody = route == Route::Clients;
                                d.child(div().pt(px(space::XS)).child(Button::new(("msg-go", i as u64)).small().label(label).on_click(
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        let _ = weak.update(cx, |app, cx| {
                                            if nobody {
                                                app.collector_filter = CollectorFilter::Nobody;
                                            }
                                            app.navigate(route, window, cx);
                                        });
                                    },
                                )))
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex().id("bell-list").gap(px(space::XS)).max_h(px(520.)).overflow_y_scroll().children(items).into_any_element()
    }
}
