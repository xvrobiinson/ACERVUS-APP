//! Letting Acervus support see the books (roadmap 46, the owner approved all
//! six decisions 2026-11-11: https://claude.ai/artifact/6vSuRTf7kHk6C6cPiLX33Q;
//! P1 of 2026-11-12 in the PWA's docs/SAAS-PLAN.md). Only the business's Owner,
//! in Settings › Business: read-only, no ID photos, for 1 hour unless they
//! pick 24 hours or 7 days (the button names the length: their explicit
//! approval), ended early with "End support access". The cloud keeps it
//! (`platform:supportAccess`, `grantSupport`, `endSupport`), audits each
//! grant, end and opening, and writes them into the business's history. While
//! it lasts the Owner and Admins see a notice on the Overview. The web's twin
//! is src/ui/support.tsx.

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::VzApp;
use crate::i18n::t;
use crate::sync::outbox::Outcome;
use crate::sync::wire::Need;
use crate::tr;
use crate::ui::components::{Tone, icon, sel, tone_colors};
use crate::ui::kit::Button;
use crate::ui::theme::{Palette, RADIUS_CONTROL, space};

/// How often the Owner's and Admins' view of support access is asked again.
const SUPPORT_REFRESH_MS: i64 = 5 * 60 * 1000;

/// How long an Owner can let support in, in hours; the first is the default.
pub(crate) const SUPPORT_HOURS: [u16; 3] = [1, 24, 168];

/// What Settings › Business shows about support access.
pub(crate) struct Support {
    /// Until when support may look (ms since 1970), as the cloud last said.
    pub until: Option<i64>,
    /// The length picked for the next time (1 hour unless changed).
    pub hours: u16,
    /// The latest time support opened the books during this access, who
    /// ("Acervus support · Ana (support)") and how many times.
    pub last_opened: Option<(i64, String)>,
    pub openings: u32,
    /// A call is on its way.
    pub busy: bool,
    /// The business last asked about, and when (ms), so it isn't asked on
    /// every frame.
    pub asked: Option<String>,
    pub asked_at: i64,
}

impl Default for Support {
    fn default() -> Self {
        Self { until: None, hours: SUPPORT_HOURS[0], busy: false, asked: None, asked_at: 0, last_opened: None, openings: 0 }
    }
}

impl Support {
    /// Whether support may look right now.
    pub fn on(&self) -> bool {
        self.until.is_some_and(|u| u > chrono::Utc::now().timestamp_millis())
    }

    /// Takes in `platform:supportAccess`'s answer (null when off).
    pub fn read(&mut self, v: &Value) {
        self.until = v.get("until").and_then(Value::as_f64).map(|u| u as i64);
        self.openings = v.get("openings").and_then(Value::as_u64).unwrap_or(0) as u32;
        self.last_opened = v.get("lastOpenedAt").and_then(Value::as_f64).map(|at| {
            let by = v.get("lastOpenedBy").and_then(Value::as_str).unwrap_or("Acervus support").to_string();
            (at as i64, by)
        });
    }
}

/// "1 hour", "24 hours", "7 days".
pub(crate) fn length_label(hours: u16) -> &'static str {
    match hours {
        1 => t("1 hour", "1 hora"),
        24 => t("24 hours", "24 horas"),
        _ => t("7 days", "7 días"),
    }
}

/// "Nov 12, 2026 15:40" in this PC's time and the business's date style.
pub(crate) fn until_label(ms: i64, style: crate::utils::dates::DateStyle) -> String {
    let Some(at) = chrono::DateTime::from_timestamp_millis(ms) else { return String::new() };
    let at = at.with_timezone(&chrono::Local);
    format!("{} {}", style.format(at.date_naive()), at.format("%H:%M"))
}

impl VzApp {
    /// The linked business, when the signed-in person is its Owner.
    pub(crate) fn owned_business(&self) -> Option<String> {
        self.cloud.as_ref().and_then(|c| c.linked.clone()).filter(|_| self.can(crate::sync::wire::Need::Owner))
    }

    /// The linked business, when the signed-in person sees its settings
    /// (the Owner and Admins), who are told while support can look.
    fn support_business(&self) -> Option<String> {
        self.cloud.as_ref().and_then(|c| c.linked.clone()).filter(|_| self.can(crate::sync::wire::Need::Settings))
    }

    /// Asks the cloud whether support can look (Settings › Business for the
    /// Owner, the Overview's notice for the Owner and Admins): once per
    /// business, then again every few minutes, since the Owner may give or end
    /// it from another device. The last answer shows meanwhile.
    pub(crate) fn load_support(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(business) = self.support_business() else { return };
        let now = chrono::Utc::now().timestamp_millis();
        if self.support.asked.as_deref() == Some(business.as_str()) {
            if self.support.busy || now - self.support.asked_at < SUPPORT_REFRESH_MS {
                return;
            }
        } else {
            self.support = Support { asked: Some(business.clone()), ..Default::default() };
        }
        self.support.asked_at = now;
        let Some(answer) = self.cloud_query("platform:supportAccess", json!({ "businessId": business })) else { return };
        cx.spawn_in(window, async move |this, cx| {
            let Outcome::Accepted(v) = answer.await.unwrap_or(Outcome::NoAnswer) else { return };
            let _ = this.update(cx, |app, cx| {
                if app.support.asked.as_deref() == Some(business.as_str()) {
                    app.support.read(&v);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The Overview's notice while support can look (P1: the business is
    /// told while access is active), for the Owner and Admins; the Owner can
    /// end it from here.
    pub(crate) fn support_banner(&self, p: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let until = self.support.until.filter(|_| self.support.on() && self.can(Need::Settings))?;
        let style = self.settings().date_style;
        let until = until_label(until, style);
        let mut line =
            tr!("Acervus support can see your books until {until}.", "El soporte de Acervus puede ver tus libros hasta el {until}.");
        if let Some((at, _)) = &self.support.last_opened {
            let when = until_label(*at, style);
            line.push(' ');
            line.push_str(&tr!("Last opened {when}.", "Abiertos por última vez el {when}."));
        }
        let (bg, ink) = tone_colors(p, Tone::Warning);
        let owner = self.can(Need::Owner);
        let online = self.cloud.as_ref().is_some_and(|c| c.online);
        Some(
            h_flex()
                .gap(px(space::M))
                .px(px(space::L))
                .py(px(space::M))
                .rounded(px(RADIUS_CONTROL))
                .bg(bg)
                .child(icon(IconName::Eye, ink, 16.))
                .child(div().flex_1().min_w_0().text_color(ink).child(sel(line)))
                .when(owner, |d| {
                    d.child(
                        Button::new("support-end-now")
                            .small()
                            .label(t("End now", "Terminar ahora"))
                            .loading(self.support.busy)
                            .disabled(!online)
                            .on_click(cx.listener(|this, _, w, cx| this.set_support(false, w, cx))),
                    )
                })
                .into_any_element(),
        )
    }

    /// Turns support access on for the length picked, or ends it.
    pub(crate) fn set_support(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(business) = self.owned_business() else { return };
        if self.support.busy {
            return;
        }
        let answer = if on {
            self.cloud_call("platform:grantSupport", json!({ "businessId": business, "hours": self.support.hours }))
        } else {
            self.cloud_call("platform:endSupport", json!({ "businessId": business }))
        };
        let Some(answer) = answer else { return };
        self.support.busy = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = answer.await.unwrap_or(Outcome::NoAnswer);
            let _ = this.update_in(cx, |app, window, cx| {
                app.support.busy = false;
                match &outcome {
                    Outcome::Accepted(v) => {
                        app.support.until = if on { v.get("until").and_then(Value::as_f64).map(|u| u as i64) } else { None };
                        app.support.last_opened = None;
                        app.support.openings = 0;
                        let msg = if on {
                            t("Acervus support can see your books.", "El soporte de Acervus puede ver tus libros.")
                        } else {
                            t("Support access ended.", "Acceso de soporte terminado.")
                        };
                        app.notify_success(msg, window, cx);
                    }
                    other => {
                        let msg = app.call_failed(other);
                        app.notify_error(msg, window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn on_only_until_its_time() {
        let now = chrono::Utc::now().timestamp_millis();
        assert!(Support { until: Some(now + 60_000), ..Default::default() }.on());
        assert!(!Support { until: Some(now - 1), ..Default::default() }.on());
        assert!(!Support::default().on());
        // P1: 1 hour unless the Owner picks longer.
        assert_eq!(Support::default().hours, 1);
    }

    #[test]
    fn reads_the_latest_opening() {
        let mut s = Support::default();
        s.read(&json!({ "until": 2.0e12, "openings": 2, "lastOpenedAt": 1.9e12, "lastOpenedBy": "Acervus support · Ana (support)" }));
        assert_eq!(s.until, Some(2_000_000_000_000));
        assert_eq!(s.openings, 2);
        assert_eq!(s.last_opened, Some((1_900_000_000_000, "Acervus support · Ana (support)".to_string())));
        s.read(&Value::Null);
        assert_eq!((s.until, s.openings, s.last_opened.clone()), (None, 0, None));
    }

    #[test]
    fn until_reads_as_a_date_and_time() {
        let label = until_label(chrono::Utc::now().timestamp_millis(), crate::utils::dates::DateStyle::Iso);
        assert!(label.len() >= 16 && label.contains(':'), "{label}");
    }
}
