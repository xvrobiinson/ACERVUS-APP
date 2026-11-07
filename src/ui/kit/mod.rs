//! The component kit: the interactive and structural pieces every screen is
//! built from. Each component reads its colors from the palette and its
//! sizes, type, depth and motion from `ui::theme`, so a screen never styles
//! a button, card or menu by hand.
//!
//! Smaller presentational helpers (pills, avatars, selectable text) live in
//! `ui::components`. See DESIGN.md for when to use what.

mod badge;
mod button;
mod card;
mod chip;
mod count_up;
mod dropdown;
mod fab;
mod glass_figure;
mod glide;
pub mod interact;
mod list_row;
mod logo;
pub mod motion;
mod overlay;
mod saved_check;
mod segmented;
mod sort_header;
mod splash;
mod stat;
mod stat_cards;
mod status_avatar;
mod theme_pill;
mod toast;

pub use badge::{Badge, BadgeTone};
pub use button::{Button, ButtonKind, ButtonSize};
pub use card::{Card, face};
pub use chip::Chip;
pub use count_up::CountUp;
pub use dropdown::{Dropdown, DropdownItem};
pub use fab::{Fab, FabItem};
pub use glass_figure::{FigureTone, GlassFigure};
pub use glide::Glide;
pub use list_row::ListRow;
pub use logo::{Logo, MarkLook, paint_mark};
pub use overlay::{Overlay, OverlayAlign};
pub use saved_check::SavedCheck;
pub use segmented::Segmented;
pub use sort_header::{Sort, SortHeader};
pub use splash::Splash;
pub use stat::Stat;
pub use stat_cards::StatCards;
pub use status_avatar::StatusAvatar;
pub use theme_pill::ThemePill;
pub use toast::Toast;

use std::rc::Rc;

use gpui_kit::*;

/// What every clickable component calls. The same shape as a GPUI click
/// listener, so `cx.listener(...)` plugs straight in.
pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
