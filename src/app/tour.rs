//! The welcome tour and Help (2026-11-10, the owner's spec; the web's
//! src/ui/tour.tsx and src/screens/help.tsx). The tour is offered once to
//! someone who hasn't finished or skipped it, on the Overview after the
//! splash; finished or skipped it stays away (kept with the account, so the
//! web app knows too), and the account panel and Settings replay it.
//! Leaving the Overview pauses it behind a "Resume tour" pill.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Route, VzApp};
use crate::i18n::t;
use crate::sync::wire::{Need, Role};
use crate::ui::kit::{Tour, TourStep};
use crate::ui::theme::{Typeset as _, palette, space, text};

/// Bumped when the tour gains steps worth showing again (the web's).
const TOUR_VERSION: u32 = 1;

impl VzApp {
    /// The tour for this person's role, from the welcome to the finish.
    fn tour_steps(&self) -> Vec<TourStep> {
        let welcome = TourStep {
            targets: &[],
            title: t("Welcome to Acervus", "Bienvenido a Acervus"),
            text: t(
                "A one-minute tour of the essentials. You can skip it now and replay it any time from your account menu.",
                "Un recorrido de un minuto por lo esencial. Puedes saltarlo ahora y repetirlo cuando quieras desde el menú de tu cuenta.",
            ),
        };
        let finish = TourStep {
            targets: &[],
            title: t("You're ready", "Todo listo"),
            text: t(
                "That's the essentials. Start with the first steps on your Overview, and find this tour again in your account menu.",
                "Eso es lo esencial. Empieza con los primeros pasos de tu Resumen, y encuentra este recorrido de nuevo en el menú de tu cuenta.",
            ),
        };
        let search = TourStep {
            targets: &["right"],
            title: t("Search and your account", "Buscar y tu cuenta"),
            text: t(
                "Find anyone by name, phone or loan number. Settings, Help and this tour are in your account menu.",
                "Encuentra a cualquiera por nombre, teléfono o número de préstamo. Ajustes, Ayuda y este recorrido están en el menú de tu cuenta.",
            ),
        };
        match self.role() {
            Some(Role::Collector) => vec![
                welcome,
                TourStep {
                    targets: &["nav-clients"],
                    title: t("Your clients", "Tus clientes"),
                    text: t(
                        "The people assigned to you, with their loans and what each one owes.",
                        "Las personas asignadas a ti, con sus préstamos y lo que debe cada una.",
                    ),
                },
                TourStep {
                    targets: &["add"],
                    title: t("Record a payment", "Registrar un pago"),
                    text: t(
                        "Press + whenever a client pays you. Choose the loan, type the amount, done.",
                        "Presiona + cada vez que un cliente te pague. Elige el préstamo, escribe el monto y listo.",
                    ),
                },
                TourStep {
                    targets: &[],
                    title: t("Pending confirmation", "Pendiente de confirmación"),
                    text: t(
                        "What you collect counts once the office confirms the money reached them. Until then the client's receipt says \u{201c}Pending confirmation\u{201d}.",
                        "Lo que cobras cuenta cuando la oficina confirma que el dinero le llegó. Hasta entonces el recibo del cliente dice \u{201c}Pendiente de confirmación\u{201d}.",
                    ),
                },
                TourStep {
                    targets: &["nav-payments"],
                    title: t("Receipts and reminders", "Recibos y recordatorios"),
                    text: t(
                        "Send a receipt or a reminder on WhatsApp from any payment or loan.",
                        "Envía un recibo o un recordatorio por WhatsApp desde cualquier pago o préstamo.",
                    ),
                },
                search,
                finish,
            ],
            Some(Role::Viewer) => vec![
                welcome,
                TourStep {
                    targets: &["figures"],
                    title: t("The business at a glance", "El negocio de un vistazo"),
                    text: t(
                        "What the business has, what is lent out and what is late. Point at the i on a card for what it means.",
                        "Lo que tiene el negocio, lo prestado y lo atrasado. Pasa el puntero por la i de una tarjeta para ver qué significa.",
                    ),
                },
                TourStep {
                    targets: &["nav-portfolio"],
                    title: t("Portfolio", "Portafolio"),
                    text: t(
                        "How the money has grown, month by month, and where it is now.",
                        "Cómo ha crecido el dinero, mes a mes, y dónde está ahora.",
                    ),
                },
                search,
                finish,
            ],
            _ => vec![
                welcome,
                TourStep {
                    targets: &["figures"],
                    title: t("Your business at a glance", "Tu negocio de un vistazo"),
                    text: t(
                        "What you have, what is lent out and what is late. Point at the i on any card for what it means.",
                        "Lo que tienes, lo prestado y lo atrasado. Pasa el puntero por la i de cualquier tarjeta para ver qué significa.",
                    ),
                },
                TourStep {
                    targets: &["today"],
                    title: t("Needs attention today", "Requiere atención hoy"),
                    text: t(
                        "What to deal with today: late loans, payments to confirm and what falls due this week.",
                        "Lo que hay que atender hoy: préstamos atrasados, pagos por confirmar y lo que vence esta semana.",
                    ),
                },
                TourStep {
                    targets: &["add"],
                    title: t("Add from anywhere", "Agrega desde cualquier lugar"),
                    text: t(
                        "The + button: a client, a loan, a payment, money in or out.",
                        "El botón +: un cliente, un préstamo, un pago, entradas o salidas de dinero.",
                    ),
                },
                TourStep {
                    targets: &["nav-clients"],
                    title: t("Clients and loans", "Clientes y préstamos"),
                    text: t(
                        "Each client has their loans; each loan its installments, payments and receipts.",
                        "Cada cliente tiene sus préstamos; cada préstamo sus cuotas, pagos y recibos.",
                    ),
                },
                TourStep {
                    targets: &["nav-payments"],
                    title: t("Payments and WhatsApp", "Pagos y WhatsApp"),
                    text: t(
                        "Record a payment in seconds, then send the receipt or a reminder on WhatsApp.",
                        "Registra un pago en segundos y luego envía el recibo o un recordatorio por WhatsApp.",
                    ),
                },
                search,
                finish,
            ],
        }
    }

    /// Starts the tour at its welcome, on the Overview.
    pub(crate) fn start_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.route != Route::Overview {
            self.navigate(Route::Overview, window, cx);
        }
        self.tour_at = Some(0);
        self.tour_settle = 2;
        self.tour_back = false;
        cx.notify();
    }

    /// Finished or skipped: kept here and with the account.
    fn end_tour(&mut self, done: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.tour_at = None;
        let value = format!("{}:{TOUR_VERSION}", if done { "done" } else { "skipped" });
        self.update_settings(|s| s.tour = value.clone(), window, cx);
        self.send_prefs(None, None, Some(value));
        cx.notify();
    }

    /// The tour over the screen while it runs (or its "Resume tour" pill),
    /// and the first offer.
    pub(crate) fn render_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.tour_targets.next_frame();
        // Offered once: someone signed in who hasn't finished or skipped it,
        // on the Overview once the splash has opened.
        if self.tour_at.is_none()
            && !self.tour_offered
            && self.prefs_known
            && self.settings().tour.is_empty()
            && self.route == Route::Overview
            && !self.motion.covering
        {
            self.tour_offered = true;
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(1400)).await;
                let _ = this.update(cx, |app, cx| {
                    if app.tour_at.is_none() && app.settings().tour.is_empty() {
                        app.tour_at = Some(0);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        let at = self.tour_at?;
        let p = palette(cx);
        if self.route != Route::Overview {
            // Back on the Overview, its marks are read a frame later.
            self.tour_settle = 2;
            return Some(
                div()
                    .absolute()
                    .left(px(24.))
                    .bottom(px(24.))
                    .child(
                        div()
                            .id("tour-resume")
                            .px(px(16.))
                            .py(px(10.))
                            .rounded_full()
                            .bg(p.text)
                            .text_color(p.surface)
                            .typeset(text::LABEL)
                            .cursor_pointer()
                            .occlude()
                            .child(t("Resume tour", "Seguir el recorrido"))
                            .on_click(cx.listener(|this, _, w, cx| this.navigate(Route::Overview, w, cx))),
                    )
                    .into_any_element(),
            );
        }
        // Each frame while it runs, so the ring follows what it points at.
        window.request_animation_frame();
        let settled = self.tour_settle == 0;
        self.tour_settle = self.tour_settle.saturating_sub(1);
        let me = cx.entity().downgrade();
        let (skip, finish) = (me.clone(), me.clone());
        Some(
            Tour::new(
                Rc::new(self.tour_steps()),
                at,
                self.tour_targets.clone(),
                move |n, _, cx| {
                    let _ = me.update(cx, |app, cx| {
                        // Back: a step with nothing to point at is skipped
                        // backwards too, never bounced forward again.
                        app.tour_back = app.tour_at.is_some_and(|a| n < a);
                        app.tour_at = Some(n);
                        cx.notify();
                    });
                },
                Rc::new(move |w, cx| {
                    let _ = skip.update(cx, |app, cx| app.end_tour(false, w, cx));
                }),
                Rc::new(move |w, cx| {
                    let _ = finish.update(cx, |app, cx| app.end_tour(true, w, cx));
                }),
            )
            .settled(settled)
            .backward(self.tour_back)
            .into_any_element(),
        )
    }

    /// Help: how Acervus works, in plain words (usability item 13).
    pub(crate) fn open_help(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let money = self.settings().money();
        let m = move |c: i64| money.format(c);
        let admin = self.can(Need::Manage);
        let me = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let p = palette(cx);
            let section = |title: &str, body: Vec<String>| {
                v_flex()
                    .gap(px(4.))
                    .child(div().typeset(text::BODY_STRONG).text_color(p.text).child(title.to_string()))
                    .children(body.into_iter().map(|b| div().typeset(text::BODY).text_color(p.muted).child(b)))
            };
            let ten = m(1_000_000);
            let (a, b, c, d) = (m(350_000), m(325_000), m(300_000), m(275_000));
            let (i, pay, next) = (m(100_000), m(600_000), m(50_000));
            let types = vec![
                format!(
                    "{} {} ",
                    crate::domain::loans::Method::Declining.suits(),
                    crate::domain::loans::Method::Declining.explanation()
                ),
                crate::tr!(
                    "Example: {ten} at 10% a month in 4 months is {a}, then {b}, {c} and {d}.",
                    "Ejemplo: {ten} al 10% mensual en 4 meses es {a}, luego {b}, {c} y {d}."
                ),
            ];
            let flat = vec![
                format!("{} {}", crate::domain::loans::Method::Flat.suits(), crate::domain::loans::Method::Flat.explanation()),
                crate::tr!("Example: {ten} at 10% a month in 4 months is {a} each month.", "Ejemplo: {ten} al 10% mensual en 4 meses es {a} cada mes."),
            ];
            let open = vec![
                format!(
                    "{} {}",
                    crate::domain::loans::Method::InterestOnly.suits(),
                    crate::domain::loans::Method::InterestOnly.explanation()
                ),
                crate::tr!(
                    "Example: {ten} at 10% a month is {i} of interest a month; paying {pay} one month leaves the next interest at {next}.",
                    "Ejemplo: {ten} al 10% mensual son {i} de interés al mes; si un mes paga {pay}, el próximo interés queda en {next}."
                ),
            ];
            let me = me.clone();
            dialog
                .title(t("How Acervus works", "Cómo funciona Acervus"))
                .w(px(640.))
                .child(
                    v_flex()
                        .id("help-body")
                        .max_h(px(560.))
                        .overflow_y_scroll()
                        .gap(px(space::L))
                        .pr(px(space::S))
                        .child(
                            gpui_kit::component::h_flex().child(
                                crate::ui::kit::Button::new("help-tour")
                                    .primary()
                                    .small()
                                    .label(t("Replay the tour", "Repetir el recorrido"))
                                    .on_click(move |_, w, cx| {
                                        w.close_dialog(cx);
                                        let _ = me.update(cx, |app, cx| app.start_tour(w, cx));
                                    }),
                            ),
                        )
                        .child(section(t("Declining balance", "Saldo decreciente"), types))
                        .child(section(t("Flat interest", "Interés fijo"), flat))
                        .child(section(t("Interest only", "Solo interés"), open))
                        .child(section(
                            t("Recording a payment", "Registrar un pago"),
                            vec![t(
                                "A payment goes to the oldest installment first: its late fee, then its interest, then its capital. The form says in plain words what the amount pays before you save, and Undo takes it back for a few seconds after.",
                                "Un pago va primero a la cuota más antigua: su mora, luego su interés y luego su capital. El formulario dice en palabras sencillas qué cubre el monto antes de guardar, y Deshacer lo revierte durante unos segundos después.",
                            )
                            .to_string()],
                        ))
                        .child(section(
                            t("Late fees", "Mora"),
                            vec![t(
                                "On installment loans, the day after a due date the part of that installment still unpaid gets the loan's late fee, once. It can be waived on the loan's page. Interest-only loans have no automatic late fee; you can add a charge to a payment.",
                                "En préstamos a cuotas, el día después de un vencimiento, la parte sin pagar de esa cuota recibe la mora del préstamo, una vez. Se puede perdonar en la página del préstamo. Los préstamos de solo interés no tienen mora automática; puedes agregar un cargo a un pago.",
                            )
                            .to_string()],
                        ))
                        .child(section(
                            t("Receipts", "Recibos"),
                            vec![t(
                                "Each business picks its payment receipt in Settings: the full schedule, or the next payment on one short page. On a receipt printer's roll (58, 80 or 112 mm), chosen in Settings › Receipts, the short one always prints.",
                                "Cada negocio elige su recibo de pago en Ajustes: el calendario completo, o el próximo pago en una página corta. En el rollo de una impresora de recibos (58, 80 o 112 mm), elegido en Ajustes › Recibos, siempre sale el corto.",
                            )
                            .to_string()],
                        ))
                        .child(section(
                            t("WhatsApp", "WhatsApp"),
                            vec![t(
                                "Remind opens WhatsApp with today's reminder written (the day before a due date, on the day, or once late); a receipt button sends the receipt as text. You press Send in WhatsApp, from your own number.",
                                "Recordar abre WhatsApp con el recordatorio de hoy escrito (el día antes del vencimiento, el día, o cuando está atrasado); el botón de recibo envía el recibo como texto. Tú presionas Enviar en WhatsApp, desde tu propio número.",
                            )
                            .to_string()],
                        ))
                        .when(admin, |d| {
                            d.child(section(
                                t("The six cards", "Las seis tarjetas"),
                                vec![t(
                                    "Point at the i beside each card's name on the Overview for what it means.",
                                    "Pasa el puntero por la i junto al nombre de cada tarjeta del Resumen para ver qué significa.",
                                )
                                .to_string()],
                            ))
                        }),
                )
        });
    }
}
