//! The person's own Overview choices that follow them to every device, the
//! same as the web app (convex/prefs.ts): the hero's four figures and the
//! period Interest earned shows. This PC keeps a copy in its settings, for
//! offline; online, the account's copy wins, and a copy made here before
//! the account had one goes up.

use gpui_kit::*;
use serde_json::{Value, json};

use super::VzApp;
use crate::domain::periods::Period;
use crate::services::hero_layout::{self, Layout};
use crate::sync::outbox::Outcome;

impl VzApp {
    /// Reads the choices kept with the account (once the cloud can answer).
    pub(crate) fn load_prefs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(answer) = self.cloud_query("prefs:mine", json!({})) else { return };
        cx.spawn_in(window, async move |this, cx| {
            let Outcome::Accepted(v) = answer.await.unwrap_or(Outcome::NoAnswer) else { return };
            let _ = this.update(cx, |app, cx| app.take_prefs(&v, cx));
        })
        .detach();
    }

    /// The account's copy replaces this PC's; with none there yet, this
    /// PC's goes up.
    fn take_prefs(&mut self, v: &Value, cx: &mut Context<Self>) {
        let mut s = self.snap.settings.clone();
        let (mut hero_up, mut period_up) = (None, None);
        match v.get("hero").and_then(Value::as_str) {
            Some(h) => s.hero_figures = h.to_string(),
            None if !s.hero_figures.is_empty() => hero_up = Some(s.hero_figures.clone()),
            None => {}
        }
        match v.get("interestPeriod").and_then(Value::as_str) {
            Some(p) => s.interest_period = Period::from_key(p),
            None => period_up = Some(s.interest_period),
        }
        if hero_up.is_some() || period_up.is_some() {
            self.send_prefs(hero_up, period_up);
        }
        if s != self.snap.settings {
            // This PC's own settings: kept here, never sent as the business's.
            if let Err(e) = self.store.save_settings(&s) {
                log::warn!("the account's Overview choices weren't kept here: {e:#}");
                return;
            }
            self.reload(cx);
        }
    }

    /// Sends a change to the account. Offline it isn't sent; this PC keeps
    /// it and the next change online carries it up.
    fn send_prefs(&self, hero: Option<String>, period: Option<Period>) {
        let mut args = serde_json::Map::new();
        if let Some(h) = hero {
            args.insert("hero".into(), h.into());
        }
        if let Some(p) = period {
            args.insert("interestPeriod".into(), p.key().into());
        }
        // The answer isn't needed: a refusal leaves this PC's copy as it is.
        drop(self.cloud_call("prefs:save", Value::Object(args)));
    }

    /// The hero's figures as this person sees them (their role decides what
    /// they may pick).
    pub(crate) fn hero_layout(&self) -> Layout {
        hero_layout::parse(&self.settings().hero_figures, self.role())
    }

    pub(crate) fn set_hero_layout(&mut self, layout: Layout, window: &mut Window, cx: &mut Context<Self>) {
        let text = hero_layout::to_text(&layout);
        self.update_settings(|s| s.hero_figures = text.clone(), window, cx);
        self.send_prefs(Some(text), None);
    }

    pub(crate) fn set_interest_period(&mut self, period: Period, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.interest_period = period, window, cx);
        self.send_prefs(None, Some(period));
    }
}
