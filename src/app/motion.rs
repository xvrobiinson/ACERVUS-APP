//! What the app remembers between frames so motion can answer a change:
//! a badge that counted up, a sync that just finished, a payment just
//! saved, a row just added, a layout just rearranged.

use std::time::{Duration, Instant};

use gpui_kit::*;

use super::VzApp;

/// A count shown on a badge, and how many times it went up (each time
/// restarts the badge's pop).
#[derive(Default)]
pub(crate) struct Counter {
    seen: Option<usize>,
    bumps: u32,
}

impl Counter {
    /// Notes the count now shown; the number of rises so far.
    pub(crate) fn see(&mut self, n: usize) -> u32 {
        if self.seen.is_some_and(|s| n > s) {
            self.bumps += 1;
        }
        self.seen = Some(n);
        self.bumps
    }
}

/// A record the person just added, whose row glows for a moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fresh {
    Payment(i64),
}

#[derive(Default)]
pub(crate) struct Motion {
    pub(crate) overdue: Counter,
    pub(crate) unread: Counter,
    /// The sync was sending changes at the last frame.
    pub(crate) sync_was_sending: bool,
    /// "Up to date" shows until then, after sending finished.
    pub(crate) up_to_date_until: Option<Instant>,
    /// The saved check, numbered so each one draws afresh.
    pub(crate) saved: Option<(u32, Instant)>,
    saved_count: u32,
    fresh: Vec<(Fresh, Instant)>,
    /// Blocks slide to their new places until then.
    glide_until: Option<Instant>,
}

/// How long a moved block may take to settle.
const GLIDE_WINDOW: Duration = Duration::from_millis(900);
/// How long "Up to date" stays after sending.
pub(crate) const UP_TO_DATE: Duration = Duration::from_secs(2);

impl Motion {
    pub(crate) fn is_fresh(&self, f: Fresh) -> bool {
        self.fresh.iter().any(|(x, at)| *x == f && at.elapsed() < crate::ui::theme::motion::GLOW)
    }

    pub(crate) fn gliding(&self) -> bool {
        self.glide_until.is_some_and(|t| Instant::now() < t)
    }

    pub(crate) fn up_to_date(&self) -> bool {
        self.up_to_date_until.is_some_and(|t| Instant::now() < t)
    }
}

impl VzApp {
    /// Draws the green check in the middle of the window for a moment.
    pub(crate) fn show_saved_check(&mut self, cx: &mut Context<Self>) {
        self.motion.saved_count += 1;
        self.motion.saved = Some((self.motion.saved_count, Instant::now()));
        let shown = self.motion.saved_count;
        self.later(crate::ui::kit::SavedCheck::SHOWN, cx, move |app| {
            if app.motion.saved.is_some_and(|(n, _)| n == shown) {
                app.motion.saved = None;
            }
        });
    }

    /// Makes the row of a record just added glow.
    pub(crate) fn mark_fresh(&mut self, f: Fresh) {
        self.motion.fresh.retain(|(_, at)| at.elapsed() < crate::ui::theme::motion::GLOW);
        self.motion.fresh.push((f, Instant::now()));
    }

    /// The layout is about to change on purpose: let blocks slide.
    pub(crate) fn arm_glide(&mut self) {
        self.motion.glide_until = Some(Instant::now() + GLIDE_WINDOW);
    }

    /// Notes the sync's state; after sending ends with nothing left, shows
    /// "Up to date" for a moment.
    pub(crate) fn note_sync(&mut self, sending: bool, synced: bool, cx: &mut Context<Self>) {
        if self.motion.sync_was_sending && !sending && synced {
            self.motion.up_to_date_until = Some(Instant::now() + UP_TO_DATE);
            self.later(UP_TO_DATE, cx, |_| {});
        }
        self.motion.sync_was_sending = sending;
    }

    /// Runs `f` after `wait`, then redraws.
    pub(crate) fn later(&self, wait: Duration, cx: &mut Context<Self>, f: impl FnOnce(&mut VzApp) + 'static) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |app, cx| {
                f(app);
                cx.notify();
            });
        })
        .detach();
    }
}
