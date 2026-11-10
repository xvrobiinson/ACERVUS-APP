//! The cloud inside the app: signing in, following a business, and keeping
//! this PC's copy and the cloud in step (docs/DESKTOP-SYNC-PLAN.md in the PWA
//! repo).
//!
//! The app needs an account (since 2026-11-04): signed out, the sign-in
//! screen takes the window. Signed in, the sync worker runs and the app
//! shows a cloud business: the one this PC opened last, or the only one;
//! with none open, the business welcome (create one, accept an invitation,
//! or choose). The open business has its own database file (a copy of the
//! cloud's books) and every save also goes to the cloud. The books kept on
//! this PC itself are never shown; once per sign-in the app offers to move
//! them into a new cloud business (`sync::import`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::StreamExt as _;
use futures::channel::oneshot;
use gpui_kit::component::input::InputState;
use gpui_kit::*;

use super::VzApp;
use crate::i18n::t;
use crate::services::{Invalid, Store};
use crate::sync::auth::{self, Endpoints};
use crate::sync::change::{Change, Saved};
use crate::sync::outbox::Outcome;
use crate::sync::replica::{self, Entity as Record};
use crate::sync::vault::{self, Stored};
use crate::sync::wire::{Membership, Need, Role, Snapshot};
use crate::sync::worker::{Config, Event, Worker};
use crate::tr;

/// The business this PC follows, kept in the PC's own books.
const LINKED: &str = "cloud_business";
/// The PC's own books went to the cloud ("done"), or the person said
/// never to ask ("never"); kept in those books.
const MOVED: &str = "moved_to_cloud";

/// Where the cloud is. Debug builds can point at a local copy of the
/// backend for testing (`ACERVUS_SYNC_CLOUD`, `ACERVUS_SYNC_ORIGIN`).
fn endpoints() -> Endpoints {
    if cfg!(debug_assertions)
        && let (Ok(cloud), Ok(origin)) = (std::env::var("ACERVUS_SYNC_CLOUD"), std::env::var("ACERVUS_SYNC_ORIGIN"))
    {
        return Endpoints::new(&cloud, &origin);
    }
    Endpoints::production()
}

/// The sign-in form in Settings.
pub(crate) struct SignIn {
    pub(crate) email: Entity<InputState>,
    pub(crate) code: Entity<InputState>,
    /// The name for a new account.
    pub(crate) name: Entity<InputState>,
    /// Making a new account (it asks for a name) rather than signing in.
    pub(crate) creating: bool,
    /// The app needs an account: nothing opens without signing in, and the
    /// screen has no way back.
    pub(crate) required: bool,
    /// The code was sent to this address; the form asks for it.
    pub(crate) code_sent_to: Option<String>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    /// A Google sign-in is waiting for the browser; setting it stops the wait.
    pub(crate) google: Option<Arc<AtomicBool>>,
    /// The sign-in screen is showing.
    pub(crate) open: bool,
}

impl SignIn {
    pub(crate) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            email: cx.new(|cx| InputState::new(window, cx)),
            code: cx.new(|cx| InputState::new(window, cx)),
            name: cx.new(|cx| InputState::new(window, cx)),
            creating: false,
            required: false,
            code_sent_to: None,
            busy: false,
            error: None,
            google: None,
            open: false,
        }
    }
}

/// What to do once the cloud has a new record and its number.
#[derive(Clone, Copy)]
enum Then {
    PaymentReceipt,
    LoanReceipt,
    /// The record's page is open under the number this PC gave it: show it
    /// under the cloud's number instead.
    Reopen(super::Route),
}

impl Then {
    fn record(self) -> Option<Record> {
        use super::Route;
        match self {
            Then::PaymentReceipt => Some(Record::Payment),
            Then::LoanReceipt | Then::Reopen(Route::Loan(_)) => Some(Record::Loan),
            Then::Reopen(Route::Client(_)) => Some(Record::Client),
            Then::Reopen(_) => None,
        }
    }
}

/// A saved change the app waits on: its operation id until the cloud
/// answers, then the record's cloud id until the copy with it arrives.
struct Waiting {
    op: String,
    remote: Option<String>,
    then: Then,
}

/// An invitation sent to the signed-in person's email, not yet accepted.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingInvite {
    pub(crate) invite_id: String,
    pub(crate) business_name: String,
    pub(crate) role: Role,
}

/// What covers the app while signed in with no business open on this PC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    /// The list of businesses hasn't come yet (or the one business is opening).
    Waiting,
    /// No business yet: create one or accept an invitation.
    Welcome,
    /// Several businesses and none chosen on this PC yet.
    Choose,
}

/// The signed-in link to the cloud.
pub(crate) struct Cloud {
    pub(crate) endpoints: Endpoints,
    pub(crate) email: String,
    worker: Worker,
    pub(crate) online: bool,
    /// The businesses the person belongs to, once the cloud has said.
    pub(crate) businesses: Option<Vec<Membership>>,
    /// The business this PC follows, if any.
    pub(crate) linked: Option<String>,
    /// The last copy of the linked business's books.
    last: Option<Box<Snapshot>>,
    sending: bool,
    /// The operation id of the change saved last (none when it wasn't sent
    /// to the cloud), for its receipt and page.
    last_op: Option<String>,
    pub(crate) update_required: Option<String>,
    /// Receipts and pages waiting for the cloud's number.
    waiting: Vec<Waiting>,
    /// The bell: this person's messages in the linked business.
    pub(crate) messages: Vec<crate::sync::wire::Message>,
    /// Backups to Google Drive.
    pub(crate) drive: super::drive::Drive,
    /// Invitations waiting for this person, once asked (welcome screen).
    pub(crate) invites: Option<Vec<PendingInvite>>,
    /// An invitation being accepted.
    pub(crate) accepting: Option<String>,
    /// Moving this PC's own books to the cloud was offered this sign-in.
    move_offered: bool,
    /// Payments the cloud took lately: the change's operation id and the
    /// payment's cloud id (a collector's: the waiting one's), for Undo
    /// (newest last).
    answered: Vec<(String, String)>,
    _events: Task<()>,
}

/// What the sidebar and Settings show about the link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// Signed in, no business chosen on this PC yet.
    NotLinked,
    Synced,
    Sending(usize),
    /// Offline; `n` changes wait to be sent.
    Offline(usize),
    /// Changes the cloud refused or may not have received.
    NeedsReview(usize),
    UpdateRequired,
}

impl VzApp {
    /// Starts syncing at launch when this PC has a remembered sign-in.
    pub(crate) fn resume_cloud(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ep = endpoints();
        let Some(stored) = vault::load(&ep.cloud) else { return };
        let linked = self.store.pref(LINKED).ok().flatten();
        self.start_cloud(ep, stored, linked, window, cx);
    }

    fn start_cloud(&mut self, ep: Endpoints, stored: Stored, linked: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let config = Config { endpoints: ep.clone(), client_version: crate::sync::CLIENT_VERSION.into() };
        let email = stored.email.clone();
        let (worker, mut rx) = match Worker::start(config, stored) {
            Ok(w) => w,
            Err(e) => {
                log::error!("sync could not start: {e}");
                return;
            }
        };
        let events = cx.spawn_in(window, async move |this, cx| {
            while let Some(ev) = rx.next().await {
                if this.update_in(cx, |app, window, cx| app.on_cloud_event(ev, window, cx)).is_err() {
                    break;
                }
            }
        });
        self.cloud = Some(Cloud {
            endpoints: ep,
            email,
            worker,
            online: false,
            businesses: None,
            linked: None,
            last: None,
            sending: false,
            last_op: None,
            update_required: None,
            waiting: Vec::new(),
            messages: Vec::new(),
            drive: Default::default(),
            invites: None,
            accepting: None,
            move_offered: false,
            answered: Vec::new(),
            _events: events,
        });
        if let Some(id) = linked {
            self.link_business(&id, window, cx);
        }
    }

    /// The last copy of the linked business from the cloud (collectors, the
    /// payments waiting for an admin), once it has arrived.
    pub(crate) fn cloud_copy(&self) -> Option<&Snapshot> {
        self.cloud.as_ref().filter(|c| c.linked.is_some())?.last.as_deref()
    }

    /// The bell's messages, newest first.
    pub(crate) fn messages(&self) -> &[crate::sync::wire::Message] {
        self.cloud.as_ref().filter(|c| c.linked.is_some()).map_or(&[], |c| c.messages.as_slice())
    }

    /// Marks the bell's messages as read (in the cloud too).
    pub(crate) fn read_messages(&mut self) {
        let Some(c) = self.cloud.as_mut() else { return };
        let Some(id) = c.linked.clone() else { return };
        if c.messages.iter().all(|m| m.read_at.is_some()) {
            return;
        }
        drop(c.worker.call("messages:markRead", serde_json::json!({ "businessId": id })));
    }

    /// True when this PC follows a business in the cloud.
    /// What covers the app instead of the books: signed in, but no business
    /// open on this PC. Never the PC's own books (the app needs a business).
    pub(crate) fn business_gate(&self) -> Option<Gate> {
        let c = self.cloud.as_ref()?;
        if c.linked.is_some() || !self.sign_in.required {
            return None;
        }
        Some(match &c.businesses {
            Some(list) if list.is_empty() => Gate::Welcome,
            Some(list) if list.len() > 1 => Gate::Choose,
            _ => Gate::Waiting,
        })
    }

    /// Debug builds: signed in as a made-up person with these businesses
    /// and invitations, for screenshots of the business welcome. The
    /// sign-in goes nowhere (a closed local port).
    pub(crate) fn demo_gate(
        &mut self,
        businesses: Vec<Membership>,
        invites: Vec<PendingInvite>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ep = Endpoints::new("http://127.0.0.1:9", "http://127.0.0.1:9");
        let stored = Stored { email: "demo@example.com".into(), session: auth::Session::from_stored("demo".into()) };
        self.start_cloud(ep, stored, None, window, cx);
        self.sign_in.required = true;
        if let Some(c) = &mut self.cloud {
            c.online = true;
            c.businesses = Some(businesses);
            c.invites = Some(invites);
        }
    }

    /// Once per sign-in: offers to move books kept only on this PC into a
    /// new cloud business, unless they were moved or the person said never.
    fn offer_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &mut self.cloud else { return };
        if c.move_offered || !self.sign_in.required {
            return;
        }
        c.move_offered = true;
        let answered = self.local_books().ok().and_then(|s| s.pref(MOVED).ok().flatten()).is_some();
        if !answered && crate::sync::import::has_records(&self.local_path) {
            self.open_move_books(window, cx);
        }
    }

    /// The books on this PC are a cloud business now: it opens, the books
    /// here are marked as moved, and their ID photos and logo follow.
    pub(crate) fn books_moved(
        &mut self,
        business: &str,
        client_ids: &serde_json::Value,
        moved: crate::sync::import::Prepared,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use base64::Engine as _;
        if let Err(e) = self.local_books().and_then(|mut s| s.set_pref(MOVED, Some("done"))) {
            log::warn!("moved books not marked: {e:#}");
        }
        self.link_business(business, window, cx);
        self.notify_success(
            t("Your books are in the cloud now, as a new business.", "Tus registros ya están en la nube, como un negocio nuevo."),
            window,
            cx,
        );
        let file = |mime: &str, bytes: &[u8]| serde_json::json!({ "contentType": mime, "data": base64::engine::general_purpose::STANDARD.encode(bytes) });
        let mut sent = Vec::new();
        for (no, mime, bytes) in &moved.photos {
            let Some(id) = client_ids.get(no.to_string()).and_then(|v| v.as_str()) else { continue };
            let args = serde_json::json!({ "businessId": business, "clientId": id, "upload": file(mime, bytes) });
            sent.extend(self.cloud_call("clients:setPhoto", args));
        }
        if let Some((mime, bytes)) = &moved.logo {
            sent.extend(self.cloud_call("settings:setLogo", serde_json::json!({ "businessId": business, "upload": file(mime, bytes) })));
        }
        cx.spawn(async move |_, _| {
            for answer in sent {
                let outcome = answer.await.unwrap_or(Outcome::NoAnswer);
                if !matches!(outcome, Outcome::Accepted(_)) {
                    log::warn!("a moved photo or logo wasn't sent: {outcome:?}");
                }
            }
        })
        .detach();
    }

    /// Never offer to move this PC's books again.
    pub(crate) fn never_move_books(&mut self) {
        if let Err(e) = self.local_books().and_then(|mut s| s.set_pref(MOVED, Some("never"))) {
            log::warn!("\"never move\" not kept: {e:#}");
        }
    }

    /// Asks the cloud for invitations sent to this person's email.
    pub(crate) fn load_invites(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(answer) = self.cloud_query("invites:pending", serde_json::json!({})) else { return };
        cx.spawn_in(window, async move |this, cx| {
            let outcome = answer.await.unwrap_or(Outcome::NoAnswer);
            let _ = this.update(cx, |app, cx| {
                if let (Outcome::Accepted(v), Some(c)) = (outcome, &mut app.cloud) {
                    c.invites = Some(serde_json::from_value(v).unwrap_or_default());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Accepts an invitation and opens its business on this PC.
    pub(crate) fn accept_invite(&mut self, invite_id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(answer) = self.cloud_call("invites:acceptPending", serde_json::json!({ "inviteId": invite_id })) else { return };
        if let Some(c) = &mut self.cloud {
            c.accepting = Some(invite_id);
        }
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = answer.await.unwrap_or(Outcome::NoAnswer);
            let _ = this.update_in(cx, |app, window, cx| {
                if let Some(c) = &mut app.cloud {
                    c.accepting = None;
                }
                match &outcome {
                    Outcome::Accepted(id) => {
                        let id = id.as_str().unwrap_or_default().to_string();
                        app.link_business(&id, window, cx);
                    }
                    _ => {
                        let why = app.call_failed(&outcome);
                        app.notify_error(why, window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn is_linked(&self) -> bool {
        self.cloud.as_ref().is_some_and(|c| c.linked.is_some())
    }

    /// The person's role in the business this PC follows; `None` on this
    /// PC's own books, where everything is allowed.
    pub(crate) fn role(&self) -> Option<Role> {
        let c = self.cloud.as_ref().filter(|c| c.linked.is_some())?;
        match &c.last {
            Some(snap) => Some(snap.me.role),
            // Before the first copy arrives: the role kept with the books.
            None => Some(self.store.cloud_link().ok().flatten().map_or(Role::Viewer, |l| l.role)),
        }
    }

    /// Whether the person may make changes that need `need`. Screens hide
    /// what it doesn't allow (the cloud refuses it anyway).
    pub(crate) fn can(&self, need: Need) -> bool {
        self.role().is_none_or(|r| r.allows(need))
    }

    /// The sync status as the database has it now (see `sync_status`).
    pub(crate) fn read_sync_status(&self) -> Option<SyncStatus> {
        let c = self.cloud.as_ref()?;
        if c.update_required.is_some() {
            return Some(SyncStatus::UpdateRequired);
        }
        if c.linked.is_none() {
            return Some(SyncStatus::NotLinked);
        }
        let refused = self.store.outbox_refused().map(|r| r.len()).unwrap_or(0);
        if refused > 0 {
            return Some(SyncStatus::NeedsReview(refused));
        }
        let waiting = self.store.outbox_waiting().map(|w| w.len()).unwrap_or(0);
        Some(match (c.online, waiting) {
            (false, n) => SyncStatus::Offline(n),
            (true, 0) => SyncStatus::Synced,
            (true, n) => SyncStatus::Sending(n),
        })
    }

    // ----- signing in -----------------------------------------------------

    pub(crate) fn send_sign_in_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sign_in.creating && self.sign_in.name.read(cx).value().trim().is_empty() {
            self.sign_in.error = Some(t("Enter your name", "Escribe tu nombre").into());
            cx.notify();
            return;
        }
        let email = self.sign_in.email.read(cx).value().trim().to_string();
        if !email.contains('@') {
            self.sign_in.error = Some(t("Enter your email address", "Escribe tu correo").into());
            cx.notify();
            return;
        }
        self.sign_in.busy = true;
        self.sign_in.error = None;
        cx.notify();
        let ep = endpoints();
        let sending = email.clone();
        let task = cx.background_spawn(async move { auth::send_code(&ep, &sending) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |app, window, cx| {
                app.sign_in.busy = false;
                match result {
                    Ok(()) => {
                        app.sign_in.code_sent_to = Some(email);
                        app.sign_in.code.update(cx, |s, cx| {
                            s.set_value("", window, cx);
                            s.focus(window, cx);
                        });
                    }
                    Err(e) => app.sign_in.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn finish_sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(email) = self.sign_in.code_sent_to.clone() else { return };
        let code: String = self.sign_in.code.read(cx).value().chars().filter(char::is_ascii_digit).collect();
        if code.len() != 6 {
            self.sign_in.error = Some(t("Enter the 6-digit code from the email", "Escribe el código de 6 dígitos del correo").into());
            cx.notify();
            return;
        }
        self.sign_in.busy = true;
        self.sign_in.error = None;
        cx.notify();
        let ep = endpoints();
        let (e2, ep2) = (email.clone(), ep.clone());
        let name = self.sign_in.creating.then(|| self.sign_in.name.read(cx).value().trim().to_string());
        let task = cx.background_spawn(async move { auth::sign_in(&ep2, &e2, &code, name.as_deref()) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |app, window, cx| {
                app.sign_in.busy = false;
                match result {
                    Ok(session) => {
                        let stored = Stored { email, session };
                        if let Err(e) = vault::save(&ep.cloud, &stored) {
                            log::warn!("sign-in not remembered: {e}");
                        }
                        app.sign_in.code_sent_to = None;
                        let linked = app.store.pref(LINKED).ok().flatten();
                        app.start_cloud(ep, stored, linked, window, cx);
                        app.signed_in(window, cx);
                    }
                    Err(e) => app.sign_in.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Signs in with Google in the person's browser; the app waits until the
    /// browser comes back (or the person cancels).
    pub(crate) fn sign_in_with_google(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sign_in.google.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.sign_in.google = Some(stop.clone());
        self.sign_in.error = None;
        cx.notify();
        let ep = endpoints();
        cx.spawn_in(window, async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            let (ep2, stop2) = (ep.clone(), stop.clone());
            // The wait can take minutes, so it gets its own thread rather than
            // one of the app's shared ones.
            let spawned = std::thread::Builder::new().name("google-sign-in".into()).spawn(move || {
                let result = auth::start_google(&ep2, &[crate::sync::drive::SCOPE]).and_then(|g| {
                    let opened = crate::shell::browse(&g.url).recv().unwrap_or_else(|_| Err("stopped".into()));
                    if let Err(why) = opened {
                        log::warn!("browser not opened: {why}");
                        return Err(auth::AuthError::Refused(t("The browser could not be opened", "No se pudo abrir el navegador").into()));
                    }
                    auth::finish_google(&ep2, g, &stop2).and_then(|done| match done {
                        auth::GoogleDone::SignedIn(email, session) => Ok((email, session)),
                        auth::GoogleDone::Linked => Err(auth::AuthError::GoogleNotFinished),
                    })
                });
                let _ = tx.send(result);
            });
            let result = match spawned {
                Ok(_) => rx.await.unwrap_or(Err(auth::AuthError::GoogleNotFinished)),
                Err(e) => Err(auth::AuthError::Refused(e.to_string())),
            };
            let _ = this.update_in(cx, |app, window, cx| {
                // Cancelled: this attempt's result no longer matters.
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                // Back from the browser: the app comes to the front.
                window.activate_window();
                app.sign_in.google = None;
                match result {
                    Ok((email, session)) => {
                        let stored = Stored { email, session };
                        if let Err(e) = vault::save(&ep.cloud, &stored) {
                            log::warn!("sign-in not remembered: {e}");
                        }
                        app.sign_in.code_sent_to = None;
                        let linked = app.store.pref(LINKED).ok().flatten();
                        app.start_cloud(ep, stored, linked, window, cx);
                        app.signed_in(window, cx);
                    }
                    Err(e) => app.sign_in.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Stops waiting for a Google sign-in.
    pub(crate) fn cancel_google(&mut self, cx: &mut Context<Self>) {
        if let Some(stop) = self.sign_in.google.take() {
            stop.store(true, Ordering::Relaxed);
        }
        cx.notify();
    }

    /// Shows the sign-in screen.
    pub(crate) fn open_sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.cloud.is_some() {
            return;
        }
        self.sign_in.open = true;
        self.sign_in.error = None;
        self.sign_in.email.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// Leaves the sign-in screen; a Google sign-in still waiting is stopped.
    pub(crate) fn close_sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sign_in.required {
            return;
        }
        self.cancel_google(cx);
        self.sign_in.open = false;
        self.sign_in.busy = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// After signing in: the launch splash, then the business this PC
    /// followed before (or the welcome while there's none yet).
    fn signed_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sign_in.open = false;
        self.sign_in.creating = false;
        self.navigate(super::Route::Overview, window, cx);
        if !cx.reduce_motion() {
            self.play_splash(cx);
        }
        window.focus(&self.focus, cx);
    }

    /// Switches between signing in and making a new account.
    pub(crate) fn set_creating(&mut self, creating: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.sign_in.creating = creating;
        self.sign_in.code_sent_to = None;
        self.sign_in.error = None;
        let field = if creating { &self.sign_in.name } else { &self.sign_in.email };
        field.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// Back to the email step (wrong address, or no code arrived).
    pub(crate) fn restart_sign_in(&mut self, cx: &mut Context<Self>) {
        self.sign_in.code_sent_to = None;
        self.sign_in.error = None;
        cx.notify();
    }

    /// Signs out on this PC: the session and the ID photos kept here are
    /// deleted, and the app goes back to the books kept on this PC. Changes
    /// not yet sent stay in the linked copy for next time.
    pub(crate) fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cloud) = self.cloud.take() else { return };
        if let Some(stored) = vault::load(&cloud.endpoints.cloud) {
            let ep = cloud.endpoints.clone();
            cx.background_spawn(async move { auth::sign_out(&ep, &stored.session) }).detach();
        }
        vault::forget(&cloud.endpoints.cloud);
        if cloud.linked.is_some() {
            if let Err(e) = self.store.forget_cloud_photos() {
                log::error!("photos not deleted on sign-out: {e:#}");
            }
            self.use_local_books(false, window, cx);
        }
        if self.sign_in.required {
            // Straight back to the sign-in screen.
            self.sign_in.open = true;
            self.sign_in.code_sent_to = None;
            self.sign_in.error = None;
            self.sign_in.email.update(cx, |s, cx| s.focus(window, cx));
        } else {
            self.notify_success(t("Signed out on this PC", "Sesión cerrada en esta PC"), window, cx);
        }
        cx.notify();
    }

    // ----- choosing a business ---------------------------------------------

    /// Switches this PC to a business's copy of the books.
    pub(crate) fn link_business(&mut self, business_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let opened = replica::path_for(business_id).and_then(Store::open);
        let store = match opened {
            Ok(s) => s,
            Err(e) => {
                let msg = tr!("Couldn't open this business's copy: {e:#}", "No se pudo abrir la copia de este negocio: {e:#}");
                self.notify_error(msg, window, cx);
                return;
            }
        };
        if let Err(e) = self.local_books().and_then(|mut s| s.set_pref(LINKED, Some(business_id))) {
            log::warn!("linked business not remembered: {e:#}");
        }
        self.swap_store(store, window, cx);
        if let Some(c) = &mut self.cloud {
            c.linked = Some(business_id.to_string());
            c.last = None;
            c.messages.clear();
            c.worker.open(business_id);
        }
        self.pump(window, cx);
    }

    /// Sends a call about businesses themselves (creating or deleting one);
    /// None when not signed in.
    pub(crate) fn cloud_call(&self, mutation: &'static str, args: serde_json::Value) -> Option<oneshot::Receiver<Outcome>> {
        self.cloud.as_ref().map(|c| c.worker.call(mutation, args))
    }

    /// Reads a query once; None when not signed in.
    pub(crate) fn cloud_query(&self, name: &'static str, args: serde_json::Value) -> Option<oneshot::Receiver<Outcome>> {
        self.cloud.as_ref().map(|c| c.worker.query(name, args))
    }

    /// Runs an action; None when not signed in.
    pub(crate) fn cloud_action(&self, name: &'static str, args: serde_json::Value) -> Option<oneshot::Receiver<Outcome>> {
        self.cloud.as_ref().map(|c| c.worker.action(name, args))
    }

    /// What the person needs to know when a call about businesses failed.
    pub(crate) fn call_failed(&self, outcome: &Outcome) -> String {
        match outcome {
            Outcome::Refused(e) => crate::sync::refusal::describe(e, self.settings()),
            _ => t(
                "No connection to the cloud. Try again when you're online.",
                "No hay conexión con la nube. Inténtalo de nuevo cuando estés en línea.",
            )
            .to_string(),
        }
    }

    /// After a business was deleted from this PC: back to the books kept
    /// here if it was the one shown.
    pub(crate) fn business_deleted(&mut self, business_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.cloud.as_ref().is_some_and(|c| c.linked.as_deref() == Some(business_id)) {
            self.use_local_books(true, window, cx);
        }
        if let Some(c) = &mut self.cloud
            && let Some(list) = &mut c.businesses
        {
            list.retain(|b| b.business_id != business_id);
        }
        self.notify_success(t("Business deleted", "Negocio eliminado"), window, cx);
    }

    /// The time zone for a new business: the open one's, else the first
    /// the person belongs to.
    pub(crate) fn new_business_timezone(&self) -> String {
        let c = self.cloud.as_ref();
        c.and_then(|c| c.last.as_ref().map(|s| s.business.timezone.clone()))
            .or_else(|| c.and_then(|c| c.businesses.as_ref()?.first().map(|b| b.timezone.clone())))
            .unwrap_or_else(|| "America/Santo_Domingo".into())
    }

    /// Stops following the cloud on this PC and shows the books kept here.
    /// `forget`: also stop opening the business at the next launch.
    pub(crate) fn use_local_books(&mut self, forget: bool, window: &mut Window, cx: &mut Context<Self>) {
        let local = match self.local_books() {
            Ok(s) => s,
            Err(e) => {
                let msg = tr!("Couldn't open the books on this PC: {e:#}", "No se pudieron abrir los registros de esta PC: {e:#}");
                self.notify_error(msg, window, cx);
                return;
            }
        };
        let mut local = local;
        if forget && let Err(e) = local.set_pref(LINKED, None) {
            log::warn!("linked business not forgotten: {e:#}");
        }
        if let Some(c) = &mut self.cloud {
            c.linked = None;
            c.last = None;
            c.messages.clear();
        }
        self.swap_store(local, window, cx);
    }

    /// The books kept on this PC (the database the app opened at launch).
    fn local_books(&self) -> anyhow::Result<Store> {
        Store::open(self.local_path.clone())
    }

    fn swap_store(&mut self, store: Store, window: &mut Window, cx: &mut Context<Self>) {
        self.store = store;
        self.history.clear();
        self.route = super::Route::Overview;
        self.reload(cx);
        self.reset_setting_inputs(window, cx);
        crate::ui::theme::apply(self.settings().theme, cx);
    }

    // ----- saving -----------------------------------------------------------

    /// Saves a change: here at once and, when this PC follows a business,
    /// for the cloud too. Offline, only the changes the cloud takes later can
    /// be saved (new clients, payments, expenses and income).
    pub(crate) fn save(&mut self, change: Change, window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Saved> {
        if let Some(c) = &self.cloud
            && c.linked.is_some()
        {
            if c.update_required.is_some() {
                return Err(Invalid(
                    t("Update the app to save changes to the cloud.", "Actualiza la app para guardar cambios en la nube.").into(),
                )
                .into());
            }
            if !c.online && !change.works_offline() {
                return Err(Invalid(
                    t(
                        "This change needs a connection to the cloud. New clients, payments, expenses and income can be saved offline.",
                        "Este cambio necesita conexión con la nube. Sin conexión puedes guardar clientes nuevos, pagos, gastos e ingresos.",
                    )
                    .into(),
                )
                .into());
            }
        }
        if !self.can(change.need()) {
            return Err(Invalid(crate::sync::refusal::forbidden().into()).into());
        }
        let (saved, queued) = self.store.save(&change, self.today)?;
        if let Some(c) = &mut self.cloud {
            c.last_op = queued.as_ref().map(|q| q.op_id.clone());
        }
        if queued.is_some() {
            self.pump(window, cx);
        }
        Ok(saved)
    }

    /// The operation id of the change saved last, when it went to the cloud.
    pub(crate) fn last_op(&self) -> Option<String> {
        self.cloud.as_ref().and_then(|c| c.last_op.clone())
    }

    /// Undo for a payment just saved (2026-11-10; the web does the same),
    /// found by the change's operation id, never by this PC's number for it
    /// (the cloud's copy can renumber payments). Not sent yet, it leaves this
    /// PC. Sent, an admin's is deleted by its cloud id; a collector's is taken
    /// back while it waits, within 10 minutes (`payments:withdraw`).
    pub(crate) fn undo_payment(&mut self, op: String, local: Option<i64>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &self.cloud else { return };
        let (sending, business, has_copy) = (c.sending, c.linked.clone(), c.last.is_some());
        let answer = c.answered.iter().rev().find(|(o, _)| *o == op).map(|(_, id)| id.clone());
        let reason = t("Undone right after recording it", "Deshecho justo después de registrarlo").to_string();
        // Not on its way right now, so it can't reach the cloud after this.
        if answer.is_none() && !sending {
            match self.store.outbox_drop_unsent(&op) {
                Ok(true) => {
                    if let Some(c) = &mut self.cloud {
                        c.waiting.retain(|w| w.op != op);
                    }
                    if has_copy {
                        // The cloud's copy, without the change.
                        self.apply_last(window, cx);
                    } else if let Some(id) = local
                        && let Err(e) = self.store.delete_payment(id, &reason, self.today)
                    {
                        log::warn!("undo: payment not removed here: {e:#}");
                    }
                    self.reload(cx);
                    self.notify_success(t("Payment undone", "Pago deshecho"), window, cx);
                    return;
                }
                Ok(false) => {}
                Err(e) => log::warn!("undo: outbox not read: {e:#}"),
            }
            if self.store.outbox_refused().is_ok_and(|r| r.iter().any(|x| x.op_id == op)) {
                self.notify_error(
                    t(
                        "The cloud didn't accept this payment, so there is nothing to undo. Review it from your account menu.",
                        "La nube no aceptó este pago, así que no hay nada que deshacer. Revísalo desde el menú de tu cuenta.",
                    ),
                    window,
                    cx,
                );
                return;
            }
        }
        let (Some(remote), Some(business)) = (answer, business) else {
            self.notify_error(
                t(
                    "It's on its way to the cloud. Try Undo again in a moment.",
                    "Va camino a la nube. Prueba Deshacer de nuevo en un momento.",
                ),
                window,
                cx,
            );
            return;
        };
        if self.role() != Some(Role::Collector) {
            let Some(id) = self.store.cloud_local_id(Record::Payment, &remote).ok().flatten() else {
                self.notify_error(t("This payment isn't on this PC any more.", "Este pago ya no está en esta PC."), window, cx);
                return;
            };
            match self.save(Change::DeletePayment { id, reason }, window, cx) {
                Ok(_) => {
                    self.reload(cx);
                    self.notify_success(t("Payment undone", "Pago deshecho"), window, cx);
                }
                Err(e) => {
                    let m = self.error_message(&e);
                    self.notify_error(m, window, cx);
                }
            }
            return;
        }
        let Some(answer) = self.cloud_call("payments:withdraw", serde_json::json!({ "businessId": business, "pendingId": remote })) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let outcome = answer.await.unwrap_or(Outcome::NotSent);
            let _ = this.update_in(cx, |app, window, cx| match outcome {
                Outcome::Accepted(_) => app.notify_success(t("Payment undone", "Pago deshecho"), window, cx),
                other => {
                    let why = app.call_failed(&other);
                    app.notify_error(why, window, cx);
                }
            });
        })
        .detach();
    }

    /// Opens a new payment's receipt: at once on this PC's own books, or once
    /// the cloud has the payment and its number.
    pub(crate) fn payment_receipt_after_save(&mut self, payment_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.receipt_after_save(payment_id, Then::PaymentReceipt, window, cx);
    }

    pub(crate) fn loan_receipt_after_save(&mut self, loan_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.receipt_after_save(loan_id, Then::LoanReceipt, window, cx);
    }

    fn receipt_after_save(&mut self, local_id: i64, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        if !self.wait_for_cloud(then) {
            self.then(then, local_id, window, cx);
            return;
        }
        if self.cloud.as_ref().is_some_and(|c| !c.online) {
            self.notify_success(
                t("The receipt will open once this reaches the cloud.", "El recibo se abrirá cuando esto llegue a la nube."),
                window,
                cx,
            );
        }
    }

    /// Opens the page of a record just saved. On a linked PC the record has
    /// this PC's number until the cloud accepts it; the page then follows it
    /// to the cloud's number (or goes back to the list if it's refused).
    pub(crate) fn open_saved(&mut self, route: super::Route, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(route, window, cx);
        self.wait_for_cloud(Then::Reopen(route));
    }

    /// Waits for the cloud's answer to the change saved last, when it went to
    /// the cloud. False when there is nothing to wait for.
    fn wait_for_cloud(&mut self, then: Then) -> bool {
        let Some(c) = self.cloud.as_mut().filter(|c| c.linked.is_some()) else { return false };
        let Some(op) = c.last_op.clone() else { return false };
        c.waiting.push(Waiting { op, remote: None, then });
        true
    }

    fn then(&mut self, then: Then, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        use super::Route;
        match then {
            Then::PaymentReceipt => self.open_receipt(id, window, cx),
            Then::LoanReceipt => self.open_loan_receipt(id, window, cx),
            Then::Reopen(old) => {
                let new = match old {
                    Route::Loan(_) => Route::Loan(id),
                    Route::Client(_) => Route::Client(id),
                    other => other,
                };
                self.replace_route(old, new);
            }
        }
    }

    /// Points the page and the back history at `new` where they showed `old`.
    fn replace_route(&mut self, old: super::Route, new: super::Route) {
        if self.route == old {
            self.route = new;
        }
        for r in self.history.iter_mut().filter(|r| **r == old) {
            *r = new;
        }
    }

    // ----- the worker's reports ---------------------------------------------

    fn on_cloud_event(&mut self, ev: Event, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &mut self.cloud else { return };
        match ev {
            Event::Online(on) => {
                c.online = on;
                if on {
                    self.pump(window, cx);
                    // Whether Drive backups are on depends on the Google
                    // connection: ask as soon as the cloud can answer.
                    self.maybe_drive_backup(window, cx);
                    self.maybe_check_update(cx);
                    self.load_prefs(window, cx);
                    self.load_profile(window, cx);
                }
            }
            Event::UpdateRequired { min_version } => c.update_required = Some(min_version),
            Event::SignedOut => {
                self.notify_error(
                    t(
                        "You were signed out. Sign in again to keep syncing.",
                        "Se cerró tu sesión. Inicia sesión de nuevo para seguir sincronizando.",
                    ),
                    window,
                    cx,
                );
                self.sign_out(window, cx);
            }
            Event::Businesses(list) => {
                // An empty list can also mean the sign-in didn't reach the
                // cloud, so only a list without this business counts.
                let gone = c.linked.as_ref().is_some_and(|id| !list.is_empty() && !list.iter().any(|b| &b.business_id == id));
                // Nothing open yet: the only business opens by itself, and
                // with none the welcome asks for invitations.
                let only = (c.linked.is_none() && list.len() == 1).then(|| list[0].business_id.clone());
                let none = c.linked.is_none() && list.is_empty();
                c.businesses = Some(list);
                if let Some(id) = only {
                    self.link_business(&id, window, cx);
                } else if none {
                    self.load_invites(window, cx);
                }
                self.offer_move(window, cx);
                if gone {
                    self.notify_error(
                        t(
                            "The business shown on this PC was deleted, or you were removed from it. This PC shows its own books again.",
                            "El negocio que se veía en esta PC se eliminó, o te quitaron de él. Esta PC vuelve a mostrar sus propios registros.",
                        ),
                        window,
                        cx,
                    );
                    self.use_local_books(true, window, cx);
                }
            }
            Event::Snapshot(snap) => {
                if c.linked.as_deref() != Some(snap.business.id.as_str()) {
                    return;
                }
                c.last = Some(snap);
                self.apply_last(window, cx);
                let Some(c) = &self.cloud else { return };
                if let Some(snap) = &c.last {
                    match self.store.cloud_downloads(snap) {
                        Ok(files) => c.worker.download(files),
                        Err(e) => log::error!("downloads not listed: {e:#}"),
                    }
                }
            }
            Event::Messages(list) => c.messages = list,
            Event::Downloaded { file, bytes } => {
                if c.linked.is_some() {
                    match self.store.save_download(&file, &bytes) {
                        Ok(()) => self.reload(cx),
                        Err(e) => log::warn!("download not kept: {e:#}"),
                    }
                }
            }
            Event::Problem(p) => log::warn!("sync: {p}"),
        }
        cx.notify();
    }

    /// Applies the last copy from the cloud (with the changes still waiting
    /// on top) and opens receipts whose records have arrived.
    fn apply_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snap) = self.cloud.as_ref().and_then(|c| c.last.clone()) else { return };
        if let Err(e) = self.store.apply_cloud(&snap) {
            log::error!("cloud copy not applied: {e:#}");
            let msg = tr!(
                "The latest books from the cloud couldn't be shown: {e:#}",
                "No se pudieron mostrar los registros más recientes de la nube: {e:#}"
            );
            self.notify_error(msg, window, cx);
            return;
        }
        self.reload(cx);
        let Some(c) = &mut self.cloud else { return };
        let mut ready = Vec::new();
        let mut pending = Vec::new();
        for w in std::mem::take(&mut c.waiting) {
            // A collector's payment: its receipt says it waits for the office.
            if matches!(w.then, Then::PaymentReceipt)
                && let Some(r) = w.remote.as_deref()
                && snap.pending.iter().any(|p| p.id == r)
            {
                pending.push(r.to_string());
                continue;
            }
            let local = w.remote.as_deref().zip(w.then.record()).and_then(|(r, e)| self.store.cloud_local_id(e, r).ok().flatten());
            match local {
                Some(id) => ready.push((w.then, id)),
                None => c.waiting.push(w),
            }
        }
        for (then, id) in ready {
            self.then(then, id, window, cx);
        }
        for id in pending {
            self.open_pending_receipt(&id, window, cx);
        }
    }

    /// Sends the oldest waiting change, then the next, until none wait or the
    /// connection is gone.
    pub(crate) fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &mut self.cloud else { return };
        if c.sending || !c.online || c.linked.is_none() || c.update_required.is_some() {
            return;
        }
        let Some(q) = self.store.outbox_waiting().ok().and_then(|w| w.into_iter().next()) else { return };
        c.sending = true;
        let answer = c.worker.send(&q);
        cx.spawn_in(window, async move |this, cx| {
            let outcome = answer.await.unwrap_or(Outcome::NotSent);
            let _ = this.update_in(cx, |app, window, cx| {
                if let Some(c) = &mut app.cloud {
                    c.sending = false;
                }
                let changed = match app.store.outbox_finish(&q, &outcome) {
                    Ok(changed) => changed,
                    Err(e) => {
                        log::error!("outbox not updated: {e:#}");
                        return;
                    }
                };
                match &outcome {
                    Outcome::Accepted(result) => {
                        if result["photoNotSaved"] == true {
                            app.notify_error(
                                t(
                                    "The client was saved, but the photo wasn't. Add it again.",
                                    "El cliente se guardó, pero la foto no. Agrégala de nuevo.",
                                ),
                                window,
                                cx,
                            );
                        }
                        // A collector's payment comes back as a pending one.
                        let remote = ["paymentId", "pendingId", "loanId", "clientId"]
                            .iter()
                            .find_map(|k| result[*k].as_str())
                            .or(result.as_str())
                            .map(str::to_string);
                        if let Some(c) = &mut app.cloud {
                            for w in c.waiting.iter_mut().filter(|w| w.op == q.op_id) {
                                w.remote = remote.clone();
                            }
                            if let Some(id) = result["paymentId"].as_str().or(result["pendingId"].as_str()) {
                                c.answered.push((q.op_id.clone(), id.to_string()));
                                let extra = c.answered.len().saturating_sub(20);
                                c.answered.drain(..extra);
                            }
                        }
                    }
                    Outcome::Refused(e) => {
                        let why = crate::sync::refusal::describe(e, app.settings());
                        app.notify_error(tr!("The cloud didn't accept a change: {why}", "La nube no aceptó un cambio: {why}"), window, cx);
                        let gone: Vec<Then> = app.cloud.as_mut().map_or(Vec::new(), |c| {
                            let (gone, keep) = std::mem::take(&mut c.waiting).into_iter().partition(|w| w.op == q.op_id);
                            c.waiting = keep;
                            gone.into_iter().map(|w: Waiting| w.then).collect()
                        });
                        // A page showing the refused record goes back to its list.
                        for then in gone {
                            if let Then::Reopen(route) = then {
                                app.replace_route(route, route.section());
                            }
                        }
                    }
                    Outcome::NoAnswer | Outcome::NotSent => {}
                }
                if changed {
                    app.apply_last(window, cx);
                }
                // Keep going while the cloud answers; a lost answer waits for
                // the next connection (or the next minute).
                if matches!(outcome, Outcome::Accepted(_) | Outcome::Refused(_)) || (changed && matches!(outcome, Outcome::NoAnswer)) {
                    app.pump(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Sends a change the cloud refused again (after the person checked it).
    pub(crate) fn retry_refused(&mut self, op_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.store.outbox_retry(op_id, None) {
            self.notify_error(format!("{e:#}"), window, cx);
            return;
        }
        self.apply_last(window, cx);
        self.pump(window, cx);
    }

    /// Drops a refused change for good.
    pub(crate) fn discard_refused(&mut self, op_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.store.outbox_discard(op_id) {
            self.notify_error(format!("{e:#}"), window, cx);
            return;
        }
        cx.notify();
    }
}
