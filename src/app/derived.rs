//! Figures worked out from the books, kept until the books or the date
//! change. Screens redraw on every animation frame and every mouse move
//! over a chart; without this each redraw would work out the totals and the
//! whole growth history again.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use chrono::NaiveDate;

use super::{SyncStatus, VzApp};
use crate::domain::portfolio::{HistoryPoint, Summary};
use crate::services::settings::ChartRange;

#[derive(Default)]
pub(crate) struct Derived {
    /// Which books and day these were worked out for.
    key: Cell<Option<(usize, NaiveDate)>>,
    summary: RefCell<Option<Rc<Summary>>>,
    history: RefCell<Vec<(ChartRange, Rc<Vec<HistoryPoint>>)>>,
    /// The sync status, read once per frame (it asks the database).
    sync: Cell<Option<Option<SyncStatus>>>,
}

impl VzApp {
    /// Forgets what was worked out when the books or the day changed.
    fn derived_fresh(&self) -> &Derived {
        let key = (Rc::as_ptr(&self.snap) as usize, self.today);
        let d = &self.derived;
        if d.key.get() != Some(key) {
            d.key.set(Some(key));
            d.summary.borrow_mut().take();
            d.history.borrow_mut().clear();
        }
        d
    }

    /// Today's totals.
    pub(crate) fn summary_now(&self) -> Rc<Summary> {
        let d = self.derived_fresh();
        d.summary.borrow_mut().get_or_insert_with(|| Rc::new(self.snap.book.summary(self.today))).clone()
    }

    /// The growth chart's points for a range.
    pub(crate) fn chart_points(&self, range: ChartRange) -> Rc<Vec<HistoryPoint>> {
        let d = self.derived_fresh();
        if let Some((_, pts)) = d.history.borrow().iter().find(|(r, _)| *r == range) {
            return pts.clone();
        }
        let pts = Rc::new(match self.snap.book.first_activity() {
            None => Vec::new(),
            Some(first) => {
                let from = match range.months() {
                    Some(m) => self.today.checked_sub_months(chrono::Months::new(m)).unwrap_or(self.today).max(first),
                    None => first,
                };
                self.snap.book.history(from.min(self.today), self.today)
            }
        });
        d.history.borrow_mut().push((range, pts.clone()));
        pts
    }

    /// The sync status for this frame.
    pub(crate) fn sync_status(&self) -> Option<SyncStatus> {
        if let Some(s) = self.derived.sync.get() {
            return s;
        }
        let s = self.read_sync_status();
        self.derived.sync.set(Some(s));
        s
    }

    /// A new frame: the sync status is read again.
    pub(crate) fn begin_frame(&self) {
        self.derived.sync.set(None);
    }
}
