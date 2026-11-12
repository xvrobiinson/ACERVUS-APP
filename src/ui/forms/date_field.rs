//! A date field in the app's language: type the date (dd/mm/yyyy in
//! Spanish, mm/dd/yyyy in English) or pick it from a small calendar. The
//! calendar's title opens the years, a year its months and a month its days,
//! so a date years back takes three clicks (owner 2026-11-11). Every date
//! field in the app is this one.

use chrono::{Datelike, NaiveDate};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n::{self, t};
use crate::ui::components::icon;
use crate::ui::kit::Button;
use crate::ui::theme::{Elevation, RADIUS_CHIP, RADIUS_CONTROL, Typeset as _, elevation, palette, space, tabular, text};
use crate::utils::dates::{add_months, input_format, input_placeholder, month_start, month_year, parse_input, today};

pub struct DateChanged;

/// What the calendar shows: a month's days, a year's months, or twelve years.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Days,
    Months,
    Years,
}

/// The first of the twelve years shown with `year`.
fn year_page(year: i32) -> i32 {
    year - year.rem_euclid(12)
}

pub struct DateField {
    input: Entity<InputState>,
    open: bool,
    month: NaiveDate,
    view: View,
    /// The first year on the years page.
    years_from: i32,
    _subs: Vec<Subscription>,
}

impl EventEmitter<DateChanged> for DateField {}

impl DateField {
    pub fn new(date: NaiveDate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let lang = i18n::get();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(input_placeholder(lang)).default_value(date.format(input_format(lang)).to_string())
        });
        let subs = vec![cx.subscribe(&input, |this, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                if let Some(d) = this.date(cx) {
                    this.month = month_start(d);
                }
                cx.emit(DateChanged);
                cx.notify();
            }
        })];
        Self { input, open: false, month: month_start(date), view: View::Days, years_from: year_page(date.year()), _subs: subs }
    }

    /// The typed date, if it is a valid date.
    pub fn date(&self, cx: &App) -> Option<NaiveDate> {
        parse_input(&self.input.read(cx).value(), i18n::get())
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }

    fn pick(&mut self, d: NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        let text = d.format(input_format(i18n::get())).to_string();
        self.input.update(cx, |s, cx| s.set_value(text, window, cx));
        self.month = month_start(d);
        self.open = false;
        cx.emit(DateChanged);
        cx.notify();
    }

    /// A year or a month was picked: show `month` on `next` (its months, or its days).
    fn choose(&mut self, month: NaiveDate, next: View, cx: &mut Context<Self>) {
        self.month = month;
        self.view = next;
        cx.notify();
    }

    /// Twelve cells, three to a row (the years and months pages).
    fn cells(&self, cx: &mut Context<Self>, cells: Vec<(SharedString, bool, bool, View, NaiveDate)>) -> Div {
        let p = palette(cx);
        let mut rows = Vec::new();
        let mut cells = cells.into_iter().enumerate().peekable();
        while cells.peek().is_some() {
            let row: Vec<_> = cells.by_ref().take(3).collect();
            rows.push(h_flex().gap(px(4.)).children(row.into_iter().map(|(i, (label, chosen, now, next, month))| {
                div()
                    .id(("cell", i as u64))
                    .flex_1()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS_CHIP))
                    .typeset(text::LABEL)
                    .font_features(tabular())
                    .cursor_pointer()
                    .text_color(if chosen { p.on_accent } else { p.text })
                    .when(chosen, |d| d.bg(p.accent))
                    .when(!chosen && now, |d| d.border_1().border_color(p.accent))
                    .when(!chosen, |d| d.hover(|s| s.bg(p.hover)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(month, next, cx)))
            })));
        }
        v_flex().gap(px(4.)).py(px(4.)).children(rows)
    }

    fn calendar(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let selected = self.date(cx);
        let now = today();
        let first = self.month;
        // Sunday-first grid covering the whole month.
        let lead = first.weekday().num_days_from_sunday() as i64;
        let grid_start = first - chrono::Duration::days(lead);
        let weeks = (0..6).map(|w| {
            h_flex().children((0..7).map(|dow| {
                let day = grid_start + chrono::Duration::days(w * 7 + dow);
                let in_month = day.month() == first.month();
                let is_sel = selected == Some(day);
                let is_today = day == now;
                div()
                    .id(("day", (w * 7 + dow) as u64))
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS_CHIP))
                    .typeset(text::CAPTION)
                    .font_features(tabular())
                    .cursor_pointer()
                    .text_color(if is_sel {
                        p.on_accent
                    } else if !in_month {
                        p.faint
                    } else {
                        p.text
                    })
                    .when(is_sel, |d| d.bg(p.accent))
                    .when(!is_sel && is_today, |d| d.border_1().border_color(p.accent))
                    .when(!is_sel, |d| d.hover(|s| s.bg(p.hover)))
                    .child(day.day().to_string())
                    .on_click(cx.listener(move |this, _, w, cx| this.pick(day, w, cx)))
            }))
        });
        // The arrows step a month, a year or twelve years, by what is shown.
        let nav_button = |id: &'static str, name: IconName, delta: i32| {
            Button::new(id).ghost().small().icon(name).on_click(cx.listener(move |this, _, _, cx| {
                match this.view {
                    View::Days | View::Months => {
                        let n = if this.view == View::Days { 1 } else { 12 };
                        this.month = if delta < 0 {
                            this.month.checked_sub_months(chrono::Months::new(n)).unwrap_or(this.month)
                        } else {
                            add_months(this.month, n)
                        };
                    }
                    View::Years => this.years_from += 12 * delta,
                }
                cx.notify();
            }))
        };
        let (prev, next) = (nav_button("prev", IconName::ChevronLeft, -1), nav_button("next", IconName::ChevronRight, 1));
        let title = match self.view {
            View::Days => {
                let title = month_year(first);
                let mut c = title.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }
            View::Months => first.year().to_string(),
            View::Years => format!("{} – {}", self.years_from, self.years_from + 11),
        };
        // The title opens the years (from the days or the months).
        let opens_years = self.view != View::Years;
        let title = div()
            .id("cal-title")
            .px(px(8.))
            .h(px(28.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(RADIUS_CHIP))
            .typeset(text::LABEL)
            .font_features(tabular())
            .child(title)
            .when(opens_years, |d| {
                d.cursor_pointer().hover(|s| s.bg(p.hover)).child(icon(IconName::ChevronDown, p.muted, 13.)).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.years_from = year_page(this.month.year());
                        this.view = View::Years;
                        cx.notify();
                    },
                ))
            });
        let chosen_year = selected.map(|d| d.year());
        let body = match self.view {
            View::Days => v_flex()
                .child(h_flex().children(i18n::weekdays().map(|w| {
                    div().size(px(32.)).flex().items_center().justify_center().typeset(text::CAPTION).text_color(p.faint).child(w)
                })))
                .children(weeks)
                .into_any_element(),
            View::Months => {
                let year = first.year();
                let cells = (1..=12)
                    .map(|m| {
                        let start = NaiveDate::from_ymd_opt(year, m, 1).unwrap_or(first);
                        let label = {
                            let s = i18n::month_short(m);
                            let mut c = s.chars();
                            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                        };
                        let chosen = selected.is_some_and(|d| d.year() == year && d.month() == m);
                        let this_month = now.year() == year && now.month() == m;
                        (label.into(), chosen, this_month, View::Days, start)
                    })
                    .collect();
                self.cells(cx, cells).into_any_element()
            }
            View::Years => {
                let cells = (self.years_from..self.years_from + 12)
                    .map(|y| {
                        let start = NaiveDate::from_ymd_opt(y, first.month(), 1).unwrap_or(first);
                        (y.to_string().into(), chosen_year == Some(y), now.year() == y, View::Months, start)
                    })
                    .collect();
                self.cells(cx, cells).into_any_element()
            }
        };
        v_flex()
            .occlude()
            .p(px(10.))
            .gap(px(6.))
            .w(px(244.))
            .bg(p.surface)
            .rounded(px(RADIUS_CONTROL + 2.))
            .border_1()
            .border_color(p.border)
            .shadow(elevation(&p, Elevation::Floating))
            .child(h_flex().justify_between().child(prev).child(title).child(next))
            .child(body)
            .child(
                div()
                    .id("today")
                    .pt(px(space::XS))
                    .typeset(text::LABEL)
                    .text_color(p.accent_text)
                    .cursor_pointer()
                    .child(t("Today", "Hoy"))
                    .on_click(cx.listener(move |this, _, w, cx| this.pick(today(), w, cx))),
            )
            .into_any_element()
    }
}

impl Render for DateField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let toggle = div()
            .id("cal-toggle")
            .flex_none()
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RADIUS_CONTROL))
            .border_1()
            .border_color(p.control)
            .cursor_pointer()
            .hover(|s| s.bg(p.hover))
            .when(self.open, |d| d.bg(p.accent_tint).border_color(p.accent))
            .child(icon(IconName::Calendar, if self.open { p.accent_text } else { p.muted }, 15.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.open = !this.open;
                    // It always opens on the days of the date in the box.
                    this.view = View::Days;
                    if let Some(d) = this.date(cx) {
                        this.month = month_start(d);
                    }
                    cx.notify();
                }),
            );
        let popup = self.open.then(|| {
            deferred(
                anchored().offset(point(px(0.), px(36.))).snap_to_window_with_margin(px(8.)).child(
                    div()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.open = false;
                            cx.notify();
                        }))
                        .child(self.calendar(cx)),
                ),
            )
            .with_priority(gpui_kit::base::POPUP_PRIORITY)
        });
        v_flex()
            .w_full()
            .child(h_flex().gap(px(6.)).child(div().flex_1().min_w_0().child(Input::new(&self.input))).child(toggle))
            .children(popup)
    }
}

#[cfg(test)]
mod tests {
    // The kit's glob import also brings GPUI's `test` macro; keep the
    // built-in one for what `#[gpui::test]` expands to.
    use ::gpui;
    use core::prelude::v1::test;

    use super::*;
    use crate::services::Store;
    use crate::ui::forms::test_app;

    #[test]
    fn the_years_page_holds_twelve_years_with_this_one() {
        assert_eq!(year_page(2026), 2016);
        assert_eq!(year_page(1990), 1980);
        assert!((year_page(1975)..year_page(1975) + 12).contains(&1975));
    }

    #[gpui::test]
    fn a_date_years_back_takes_a_year_a_month_and_a_day(cx: &mut TestAppContext) {
        let (_app, cx) = test_app(cx, Store::in_memory());
        let today = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let field = cx.update(|window, cx| crate::ui::forms::date_field(today, window, cx));
        let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        cx.update(|_, cx| {
            field.update(cx, |f, cx| {
                // Three pages back from 2026's reaches 1990.
                f.years_from = year_page(f.month.year()) - 36;
                f.view = View::Years;
                assert!((f.years_from..f.years_from + 12).contains(&1990));
                // 1990, then March, then the 14th.
                f.choose(ymd(1990, 11, 1), View::Months, cx);
                assert_eq!((f.view, f.month), (View::Months, ymd(1990, 11, 1)));
                f.choose(ymd(1990, 3, 1), View::Days, cx);
                assert_eq!((f.view, f.month), (View::Days, ymd(1990, 3, 1)));
            })
        });
        cx.update(|window, cx| field.update(cx, |f, cx| f.pick(ymd(1990, 3, 14), window, cx)));
        assert_eq!(cx.update(|_, cx| field.read(cx).date(cx)), Some(ymd(1990, 3, 14)));
    }
}
