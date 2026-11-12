//! Clickable cards (owner 2026-11-11, the list at
//! https://claude.ai/artifact/Y6XeA1qen3vwicsH521M7p): each card with a figure
//! opens the place that explains it, already filtered: a Loans filter, a
//! Payments period, a Calendar view, a part of Portfolio, a part of the page
//! the card is on, or Record payment with an amount filled in.
//!
//! Parts of a page are anchors: `anchor` records where one was drawn, and
//! `scroll_to` moves the page there (at once when it is on screen, after the
//! next frame when the card opened another page).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::*;

use super::{LoanFilter, Route, VzApp};
use crate::domain::money::Cents;
use crate::domain::periods::Period;
use crate::ui::kit::Callback;
use crate::ui::screens::calendar::CalendarView;

/// Where a card goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardLink {
    Route(Route),
    Loans(LoanFilter),
    /// Loans on a tab, filtered to one status (`list_filters::LOAN_STATUSES`).
    LoanStatus(LoanFilter, &'static str),
    /// Payments in a period (All: every payment).
    Payments(Period),
    Calendar(CalendarView),
    /// A part of a page: Portfolio's money movements, or the open page's.
    Section(Option<Route>, &'static str),
    /// Record payment on a loan with the amount filled in (not saved).
    Pay(i64, Cents),
    /// The open loan's schedule, scrolled to this row (its first late one).
    Schedule(Option<usize>),
}

/// Where each anchor was last drawn (window coordinates).
pub(crate) type Anchors = Rc<RefCell<HashMap<&'static str, Bounds<Pixels>>>>;

/// How many frames a card waits for the part it asked for (about 2 s).
const ANCHOR_FRAMES: u8 = 120;

/// Room left above a part scrolled to (px).
const ABOVE: f32 = 12.0;

impl VzApp {
    /// What a card does when clicked.
    pub(crate) fn link(&self, to: CardLink, cx: &Context<Self>) -> Callback {
        let app = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let _ = app.update(cx, |app, cx| app.open_link(to, window, cx));
        })
    }

    pub(crate) fn open_link(&mut self, to: CardLink, window: &mut Window, cx: &mut Context<Self>) {
        match to {
            CardLink::Route(r) => self.navigate(r, window, cx),
            // A card opens Loans on what it promised: a saved "Filter by"
            // from earlier would narrow it to something else.
            CardLink::Loans(filter) => {
                self.loan_filter = filter;
                self.update_settings(|s| s.loans_filter = Default::default(), window, cx);
                self.navigate(Route::Loans, window, cx);
            }
            CardLink::LoanStatus(filter, status) => {
                self.loan_filter = filter;
                let by = crate::services::list_filters::LoansBy { statuses: vec![status.to_string()], ..Default::default() };
                self.update_settings(|s| s.loans_filter = by, window, cx);
                self.navigate(Route::Loans, window, cx);
            }
            CardLink::Payments(period) => {
                let when = crate::services::list_filters::When::of(period);
                self.update_settings(
                    |s| s.payments_filter = crate::services::list_filters::PaymentsBy { when, ..Default::default() },
                    window,
                    cx,
                );
                self.navigate(Route::Payments, window, cx);
            }
            CardLink::Calendar(view) => {
                self.calendar_view = view;
                self.calendar_anchor = None;
                self.calendar_picked = None;
                self.navigate(Route::Calendar, window, cx);
            }
            CardLink::Section(route, name) => {
                if let Some(r) = route.filter(|r| *r != self.route) {
                    self.navigate(r, window, cx);
                    self.pending_anchor = Some((name, ANCHOR_FRAMES));
                    cx.notify();
                } else {
                    self.scroll_to(name, window, cx);
                }
            }
            CardLink::Schedule(row) => {
                self.scroll_to("schedule", window, cx);
                if let Some(row) = row {
                    self.schedule_scroll.scroll_to_item(row);
                }
            }
            CardLink::Pay(loan, amount) => {
                let form = self.payment_dialog(Some(loan), None, window, cx);
                if amount > 0 {
                    let typed = crate::domain::money::MoneyFormat::input_value(amount);
                    form.update(cx, |f, cx| f.type_amount(typed, window, cx));
                }
            }
        }
    }

    /// Wraps a part of a page so a card can scroll to it.
    pub(crate) fn anchor(&self, name: &'static str, el: impl IntoElement) -> AnyElement {
        let anchors = self.anchors.clone();
        div()
            .relative()
            .child(el)
            .child(
                canvas(move |bounds, _, _| anchors.borrow_mut().insert(name, bounds), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .into_any_element()
    }

    /// Scrolls the page so the part `name` starts near the top.
    pub(crate) fn scroll_to(&mut self, name: &'static str, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.anchors.borrow().get(name).copied() else { return };
        let page = self.page_scroll.bounds();
        let now = self.page_scroll.offset();
        let max = self.page_scroll.max_offset().y;
        let y = (now.y - (at.top() - page.top()) + px(ABOVE)).min(px(0.)).max(-max);
        self.page_scroll.set_offset(point(now.x, y));
        cx.notify();
    }

    /// Called while drawing: a part asked for by a card on another page is
    /// scrolled to once that page has been drawn.
    pub(crate) fn settle_pending_anchor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((name, left)) = self.pending_anchor else { return };
        if self.anchors.borrow().contains_key(name) {
            self.pending_anchor = None;
            self.scroll_to(name, window, cx);
        } else if left == 0 {
            // Never drawn (the page is covered, or went elsewhere): give up
            // rather than redraw at full speed.
            self.pending_anchor = None;
        } else {
            self.pending_anchor = Some((name, left - 1));
            window.request_animation_frame();
        }
    }
}
