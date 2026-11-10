//! Sending on WhatsApp from this PC: the app opens WhatsApp (the regular
//! one, through wa.me: WhatsApp Desktop, or WhatsApp Web in the browser)
//! with the client's number and the message written; the person presses
//! Send. A reminder is marked sent for the day on this PC.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::VzApp;
use crate::i18n::t;
use crate::services::whatsapp::{self, Reminder};
use crate::tr;

impl VzApp {
    /// The reminders to send today, as this person may (their clients only
    /// for a collector: the books hold no others).
    pub(crate) fn wa_reminders(&self) -> Vec<Reminder> {
        whatsapp::reminders(&self.snap, self.today)
    }

    /// Whether this reminder was sent from this PC today.
    pub(crate) fn wa_sent(&self, r: &Reminder) -> bool {
        self.settings().wa_sent.contains(&r.key(self.today))
    }

    /// Opens WhatsApp with the reminder written, and marks it sent.
    pub(crate) fn send_reminder(&mut self, r: &Reminder, window: &mut Window, cx: &mut Context<Self>) {
        let Some(phone) = r.phone.clone() else { return };
        let text = whatsapp::reminder_text(r, &self.snap, &self.business_name());
        self.open_whatsapp(whatsapp::link(&phone, &text), window, cx);
        let (key, today) = (r.key(self.today), self.today.to_string());
        self.update_settings(
            |s| {
                // Only today's marks are kept.
                s.wa_sent.retain(|k| k.starts_with(&today));
                if !s.wa_sent.contains(&key) {
                    s.wa_sent.push(key.clone());
                }
            },
            window,
            cx,
        );
    }

    /// Opens WhatsApp with a payment's receipt written, to its client.
    pub(crate) fn send_receipt(&mut self, payment_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let phone = self
            .snap
            .payment(payment_id)
            .and_then(|p| self.snap.loan(p.loan_id))
            .and_then(|l| self.snap.client(l.client_id))
            .and_then(|c| c.phone.as_deref())
            .and_then(|p| whatsapp::phone_for_link(p, &self.settings().currency_code));
        let Some(phone) = phone else {
            self.notify_error(
                t("This client has no phone number to send to.", "Este cliente no tiene un teléfono al que enviar."),
                window,
                cx,
            );
            return;
        };
        let Some(text) = whatsapp::receipt_text(&self.snap, payment_id) else { return };
        self.open_whatsapp(whatsapp::link(&phone, &text), window, cx);
    }

    fn open_whatsapp(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        let rx = crate::shell::browse(&url);
        cx.spawn_in(window, async move |this, cx| {
            if let Err(why) = crate::shell::wait(rx, cx.background_executor().clone()).await {
                let _ = this.update_in(cx, |app, window, cx| {
                    app.notify_error(tr!("Windows couldn't open WhatsApp ({why}).", "Windows no pudo abrir WhatsApp ({why})."), window, cx)
                });
            }
        })
        .detach();
    }

    /// A client's number ready for a wa.me link, when they have one.
    pub(crate) fn wa_phone(&self, client_id: i64) -> Option<String> {
        let raw = self.snap.client(client_id)?.phone.as_deref()?;
        whatsapp::phone_for_link(raw, &self.settings().currency_code)
    }

    /// Today's reminder for one loan, or the most urgent of a client's.
    pub(crate) fn wa_reminder_for(&self, client_id: i64, loan_id: Option<i64>) -> Option<Reminder> {
        self.wa_reminders().into_iter().find(|r| r.client_id == client_id && loan_id.is_none_or(|l| r.loan_id == l))
    }

    /// The loan or client page's WhatsApp button: it writes today's
    /// reminder when one is due (and marks it sent), otherwise it opens the
    /// chat with nothing written.
    pub(crate) fn whatsapp_client(&mut self, client_id: i64, loan_id: Option<i64>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(r) = self.wa_reminder_for(client_id, loan_id).filter(|r| r.phone.is_some()) {
            return self.send_reminder(&r, window, cx);
        }
        let Some(phone) = self.wa_phone(client_id) else {
            self.notify_error(
                t("This client has no phone number to send to.", "Este cliente no tiene un teléfono al que enviar."),
                window,
                cx,
            );
            return;
        };
        self.open_whatsapp(format!("https://wa.me/{phone}"), window, cx);
    }

    /// The WhatsApp button for a loan or client page header, when this
    /// person may send and the client has a number. It says what it does:
    /// a reminder due today, one already sent, or just the chat.
    pub(crate) fn whatsapp_button(&self, client_id: i64, loan_id: Option<i64>, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.whatsapp_button_among(&self.wa_reminders(), client_id, loan_id, cx)
    }

    /// The same, with today's reminders worked out once by the caller (a
    /// list with a button on every row).
    pub(crate) fn whatsapp_button_among(
        &self,
        reminders: &[Reminder],
        client_id: i64,
        loan_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        use crate::sync::wire::Need;
        use crate::ui::kit::Button;
        use gpui_kit::assets::IconName;
        if !self.can(Need::Record) || self.wa_phone(client_id).is_none() {
            return None;
        }
        let reminder = reminders.iter().find(|r| r.client_id == client_id && loan_id.is_none_or(|l| r.loan_id == l)).cloned();
        let sent = reminder.as_ref().is_some_and(|r| self.wa_sent(r));
        let tip = match (&reminder, sent) {
            (Some(_), false) => t("Open WhatsApp with today's reminder written", "Abrir WhatsApp con el recordatorio de hoy escrito"),
            (Some(_), true) => t("Reminder sent today. Send it again", "Recordatorio enviado hoy. Enviarlo de nuevo"),
            (None, _) => t("Open a WhatsApp chat with this client", "Abrir un chat de WhatsApp con este cliente"),
        };
        Some(
            Button::new(("wa-page", loan_id.unwrap_or(client_id) as u64))
                .ghost()
                .map(|b| if sent { b.icon(IconName::CheckCheck) } else { b.prefix(img(crate::ui::pictures::whatsapp()).size(px(15.))) })
                .label(if reminder.is_some() && !sent { t("Remind", "Recordar") } else { t("WhatsApp", "WhatsApp") })
                .tooltip(tip)
                .on_click(cx.listener(move |this, _, w, cx| this.whatsapp_client(client_id, loan_id, w, cx)))
                .into_any_element(),
        )
    }

    /// A row's "receipt on WhatsApp" button, when this person may send, the
    /// payment stands and its client has a number.
    pub(crate) fn wa_receipt_button(&self, payment_id: i64, cx: &mut Context<Self>) -> Option<crate::ui::kit::Button> {
        use crate::sync::wire::Need;
        let pay = self.snap.payment(payment_id).filter(|p| !p.is_reversed())?;
        let client_id = self.snap.loan(pay.loan_id)?.client_id;
        if !self.can(Need::Record) || self.wa_phone(client_id).is_none() {
            return None;
        }
        Some(
            crate::ui::kit::Button::new(("wa-receipt-row", payment_id as u64))
                .ghost()
                .small()
                .prefix(img(crate::ui::pictures::whatsapp()).size(px(15.)))
                .tooltip(t("Send the receipt on WhatsApp", "Enviar el recibo por WhatsApp"))
                .on_click(cx.listener(move |this, _, w, cx| {
                    cx.stop_propagation();
                    this.send_receipt(payment_id, w, cx)
                })),
        )
    }
}
