//! The business shown and its product mark: the hero's business name and
//! the switcher it opens (the sidebar is gone since the hero rollout,
//! 2026-11-06), and the mark the sign-in screen shows.

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::*;

use super::VzApp;
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::theme::{Palette, RADIUS_CONTROL, Typeset as _, space, text};

/// The product mark: the ring and dot on its black tile (`kit::Logo`).
pub(crate) fn mark(p: &Palette) -> crate::ui::kit::Logo {
    let logo = crate::ui::kit::Logo::new(24.);
    match p.splash.tile_line {
        Some(edge) => logo.edge(edge),
        None => logo,
    }
}

impl VzApp {
    /// The business shown and its switcher: its name, whether there are
    /// businesses to switch to, and (while open) the menu listing them with
    /// New business. Used by the hero's business name.
    /// The open business's name (the app's name before one is open).
    pub(crate) fn business_name(&self) -> String {
        let c = self.cloud.as_ref();
        let open_id = c.and_then(|c| c.linked.as_ref());
        c.and_then(|c| c.businesses.as_ref())
            .and_then(|l| l.iter().find(|b| Some(&b.business_id) == open_id))
            .map_or_else(|| crate::APP_NAME.to_string(), |b| b.name.clone())
    }

    pub(crate) fn brand_switcher(&self, p: &Palette, cx: &mut Context<Self>) -> (SharedString, bool, Option<crate::ui::kit::Overlay>) {
        // The business shown; clicking its name or logo lists the others to
        // switch to, and New business (approved 2026-11-05).
        let open_id = self.cloud.as_ref().and_then(|c| c.linked.clone());
        let list: Vec<(String, String)> = self
            .cloud
            .as_ref()
            .and_then(|c| c.businesses.as_ref())
            .map(|l| l.iter().map(|b| (b.business_id.clone(), b.name.clone())).collect())
            .unwrap_or_default();
        let online = self.cloud.as_ref().is_some_and(|c| c.online);
        let title = SharedString::from(self.business_name());
        let switchable = !list.is_empty();
        let biz_menu = (self.brand_open && switchable).then(|| {
            let row = |id: ElementId, label: String, mark: Option<IconName>, on: bool| {
                h_flex()
                    .id(id)
                    .h(px(30.))
                    .px(px(space::S))
                    .gap(px(space::S))
                    .rounded(px(RADIUS_CONTROL - 2.))
                    .cursor_pointer()
                    .hover(|d| d.bg(p.hover))
                    .typeset(text::LABEL)
                    .text_color(p.text)
                    .children(mark.map(|m| icon(m, p.muted, 15.)))
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .children(on.then(|| icon(IconName::Check, p.accent_text, 14.)))
            };
            let mut rows: Vec<AnyElement> = list
                .iter()
                .enumerate()
                .map(|(i, (id, name))| {
                    let (id, on) = (id.clone(), Some(id) == open_id.as_ref());
                    row(("biz-row", i as u64).into(), name.clone(), None, on)
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.brand_open = false;
                            if !on {
                                this.link_business(&id, w, cx);
                            }
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            if online {
                rows.push(
                    row("biz-new".into(), t("New business", "Nuevo negocio").into(), Some(IconName::Plus), false)
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.brand_open = false;
                            this.open_new_business(w, cx);
                        }))
                        .into_any_element(),
                );
            }
            let close = cx.entity().downgrade();
            crate::ui::kit::Overlay::new("biz-menu", move |_, cx| {
                let _ = close.update(cx, |this, cx| {
                    this.brand_open = false;
                    cx.notify();
                });
            })
            .min_width(220.)
            .child(
                div()
                    .px(px(space::S))
                    .pt(px(4.))
                    .pb(px(2.))
                    .typeset(text::MICRO)
                    .text_color(p.faint)
                    .child(t("Switch business", "Cambiar de negocio").to_uppercase()),
            )
            .children(rows)
        });
        (title, switchable, biz_menu)
    }
}
