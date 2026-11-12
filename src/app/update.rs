//! Whether a newer version is out: asked of the Acervus site while signed
//! in and online, at launch and every few hours (`sync::update`). The
//! sidebar's foot shows "Update available" only then, and opens the window
//! with what's new and "Update now".

use std::time::{Duration, Instant};

use gpui_kit::*;

use super::VzApp;
use crate::sync::update::{self, Release};

/// How often to ask again.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// After a failed look, the wait before the next try.
const RETRY: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
pub(crate) struct Updates {
    /// A newer version, once found.
    pub(crate) available: Option<Release>,
    asked: Option<(Instant, bool)>,
    asking: bool,
    /// What Settings › About's "Check for updates" found last.
    pub(crate) manual: Option<Manual>,
}

/// The answer to "Check for updates" (Settings › About).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Manual {
    Checking,
    UpToDate,
    /// Signed out or offline: nothing to ask.
    Offline,
    Failed,
}

impl VzApp {
    /// Settings › About's "Check for updates": asks now; a newer version
    /// opens the "Update available" window, otherwise About says why not.
    pub(crate) fn check_update_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.updates.asking {
            return;
        }
        let Some(ep) = self.cloud.as_ref().filter(|c| c.online).map(|c| c.endpoints.clone()) else {
            self.updates.manual = Some(Manual::Offline);
            cx.notify();
            return;
        };
        self.updates.asking = true;
        self.updates.manual = Some(Manual::Checking);
        cx.notify();
        let task = cx.background_spawn(async move { update::check(&ep) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |app, window, cx| {
                app.updates.asking = false;
                app.updates.asked = Some((Instant::now(), result.is_ok()));
                match result {
                    Ok(Some(found)) => {
                        app.updates.available = Some(found);
                        app.updates.manual = None;
                        app.open_update(window, cx);
                    }
                    Ok(None) => app.updates.manual = Some(Manual::UpToDate),
                    Err(e) => {
                        log::info!("update check: {e}");
                        app.updates.manual = Some(Manual::Failed);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks for a newer version when it's time (called every minute and
    /// when the cloud comes online).
    pub(crate) fn maybe_check_update(&mut self, cx: &mut Context<Self>) {
        let Some(c) = &self.cloud else { return };
        let due = match self.updates.asked {
            None => true,
            Some((at, ok)) => at.elapsed() >= if ok { EVERY } else { RETRY },
        };
        if !c.online || self.updates.asking || !due {
            return;
        }
        self.updates.asking = true;
        let ep = c.endpoints.clone();
        let task = cx.background_spawn(async move { update::check(&ep) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                app.updates.asking = false;
                app.updates.asked = Some((Instant::now(), result.is_ok()));
                match result {
                    Ok(found) => {
                        if found.is_some() && app.updates.available != found {
                            log::info!("a newer version is out: {:?}", found.as_ref().map(|r| &r.version));
                        }
                        app.updates.available = found;
                    }
                    Err(e) => log::info!("update check: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
