//! Settings › WhatsApp messages (2026-11-10, the owner's request; the web's
//! `WhatsAppCard`): the business's own wording for the three reminders and
//! a line before and after a receipt's figures. One wording for the whole
//! business, kept with the receipt wording, so every phone and PC sends the
//! same. Tags are filled in when a message is sent; each box shows a preview
//! with sample figures.

use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{error_banner, field, footer};
use crate::app::VzApp;
use crate::i18n::t;
use crate::services::receipt_text::{ReceiptText, WA_TAGS, WaMsg, fill_wa};
use crate::services::settings::Settings;
use crate::ui::kit::Button;
use crate::ui::theme::{Typeset as _, palette, space, text};

impl WaMsg {
    pub fn label(self) -> &'static str {
        match self {
            WaMsg::Tomorrow => t("The day before", "El día antes"),
            WaMsg::Today => t("On the due day", "El día que vence"),
            WaMsg::Late => t("When it's late", "Cuando está atrasado"),
            WaMsg::ReceiptTop => t("Before the receipt's figures", "Antes de los datos del recibo"),
            WaMsg::ReceiptEnd => t("After the receipt's figures", "Después de los datos del recibo"),
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            WaMsg::Tomorrow => t("Sent the day before an installment is due.", "Se envía el día antes de que venza una cuota."),
            WaMsg::Today => t("Sent on the day it's due.", "Se envía el día que vence."),
            WaMsg::Late => {
                t("{amount} is everything behind; {date} is since when.", "{amount} es todo lo atrasado; {date} es desde cuándo.")
            }
            WaMsg::ReceiptTop => t("Empty: the receipt starts with its number.", "Vacío: el recibo empieza con su número."),
            WaMsg::ReceiptEnd => t("Empty: nothing after the figures.", "Vacío: nada después de los datos."),
        }
    }
}

/// What each tag stands for.
pub fn tag_name(tag: &str) -> &'static str {
    match tag {
        "client" => t("client's first name", "nombre del cliente"),
        "amount" => t("amount", "monto"),
        "date" => t("date", "fecha"),
        "loan" => t("loan number", "número de préstamo"),
        _ => t("business name", "nombre del negocio"),
    }
}

/// Sample figures for the previews.
pub fn sample_tags(settings: &Settings, business: &str, today: chrono::NaiveDate) -> Vec<(&'static str, String)> {
    vec![
        ("client", "Marta".to_string()),
        ("amount", settings.money().format(150_000)),
        ("date", settings.date_style.format(today)),
        ("loan", "12".to_string()),
        ("business", business.to_string()),
    ]
}

pub struct WaTextForm {
    app: WeakEntity<VzApp>,
    base: ReceiptText,
    boxes: Vec<Entity<TextareaState>>,
    /// The box written in last: where a tag goes.
    last: usize,
    sample: Vec<(&'static str, String)>,
    error: Option<String>,
    saved: bool,
    _subs: Vec<Subscription>,
}

impl WaTextForm {
    pub fn new(
        app: WeakEntity<VzApp>,
        settings: &Settings,
        business: &str,
        today: chrono::NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let base = settings.receipt_text.clone();
        let mut subs = Vec::new();
        let boxes: Vec<_> = WaMsg::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let rows = if matches!(m, WaMsg::ReceiptTop | WaMsg::ReceiptEnd) { 2 } else { 4 };
                let state = cx.new(|cx| TextareaState::new(window, cx).rows(rows).default_value(base.wa(*m)));
                subs.push(cx.subscribe_in(&state, window, move |this: &mut Self, _, ev: &InputEvent, _, cx| match ev {
                    InputEvent::Change => cx.notify(),
                    InputEvent::Focus => this.last = i,
                    _ => {}
                }));
                state
            })
            .collect();
        Self { app, base, boxes, last: 0, sample: sample_tags(settings, business, today), error: None, saved: false, _subs: subs }
    }

    pub fn first(&self) -> &Entity<TextareaState> {
        &self.boxes[0]
    }

    fn typed(&self, i: usize, cx: &App) -> String {
        self.boxes[i].read(cx).value().to_string()
    }

    fn wording(&self, cx: &App) -> ReceiptText {
        let mut text = self.base.clone();
        for (i, m) in WaMsg::ALL.iter().enumerate() {
            let v = self.typed(i, cx).replace("\r\n", "\n").trim().to_string();
            text.wa[i] = (v != m.default_text()).then_some(v);
        }
        text
    }

    fn insert_tag(&mut self, tag: &str, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.boxes[self.last].clone();
        target.update(cx, |s, cx| {
            s.insert(format!("{{{tag}}}"), window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    fn reset(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let text = WaMsg::ALL[i].default_text();
        self.boxes[i].update(cx, |s, cx| s.set_value(text, window, cx));
        cx.notify();
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saved {
            return;
        }
        let text = self.wording(cx);
        if let Some(problem) = text.problem() {
            self.error = Some(problem.to_string());
            return cx.notify();
        }
        let result = self.app.update(cx, |app, cx| {
            let mut s = app.settings().clone();
            s.receipt_text = text;
            match app.save_settings(s, window, cx) {
                Ok(()) => {
                    app.reload(cx);
                    Ok(())
                }
                Err(e) => Err(app.error_message(&e)),
            }
        });
        match result {
            Ok(Ok(())) => {
                self.saved = true;
                window.close_dialog(cx);
                let _ = self
                    .app
                    .update(cx, |app, cx| app.notify_success(t("WhatsApp messages saved", "Mensajes de WhatsApp guardados"), window, cx));
            }
            Ok(Err(why)) => {
                self.error = Some(why);
                cx.notify();
            }
            Err(_) => window.close_dialog(cx),
        }
    }
}

impl Render for WaTextForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let tags = h_flex().flex_wrap().gap(px(6.)).children(WA_TAGS.iter().map(|tag| {
            let tag = *tag;
            Button::new(SharedString::from(format!("wa-tag-{tag}")))
                .small()
                .label(format!("{{{tag}}}"))
                .tooltip(tag_name(tag))
                .on_click(cx.listener(move |this, _, w, cx| this.insert_tag(tag, w, cx)))
        }));
        let group = |title: &'static str| div().pt(px(space::S)).typeset(text::BODY_STRONG).text_color(p.text).child(title);
        let boxes: Vec<AnyElement> = WaMsg::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let typed = self.typed(i, cx);
                let preview = fill_wa(&typed, &self.sample);
                let custom = typed.trim() != m.default_text();
                v_flex()
                    .gap(px(4.))
                    .child(field(&p, m.label(), Textarea::new(&self.boxes[i]), Some(m.hint().to_string()), None))
                    .child(
                        h_flex()
                            .gap(px(space::S))
                            .items_start()
                            .child(
                                div().flex_1().min_w_0().typeset(text::CAPTION).text_color(p.muted).child(if preview.trim().is_empty() {
                                    t("(nothing)", "(nada)").to_string()
                                } else {
                                    preview
                                }),
                            )
                            .when(custom, |r| {
                                r.child(
                                    Button::new(("wa-reset", i as u64))
                                        .ghost()
                                        .small()
                                        .label(t("Back to the built-in text", "Volver al texto predeterminado"))
                                        .on_click(cx.listener(move |this, _, w, cx| this.reset(i, w, cx))),
                                )
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        let mut boxes = boxes.into_iter();
        v_flex()
            .gap(px(space::M))
            .children(error_banner(&p, &self.error))
            .child(div().typeset(text::CAPTION).text_color(p.muted).child(t(
                "What your clients get when you press WhatsApp. One wording for the whole business; every phone and PC sends it. Click a tag to put it where you're writing; it's filled in when the message is sent.",
                "Lo que reciben tus clientes cuando pulsas WhatsApp. Un mismo texto para todo el negocio; cada teléfono y PC lo envía. Haz clic en una etiqueta para ponerla donde escribes; se llena al enviar el mensaje.",
            )))
            .child(tags)
            .child(group(t("Reminders", "Recordatorios")))
            .children(boxes.by_ref().take(3))
            .child(group(t("Receipts", "Recibos")))
            .children(boxes)
            .child(footer(&p, t("Save messages", "Guardar mensajes"), true, false, cx.listener(|this, _, w, cx| this.submit(w, cx))))
    }
}
