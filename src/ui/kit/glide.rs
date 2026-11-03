//! Glide: when an element's place in the layout changes (a block moved on
//! the Overview), it slides from where it was to where it now is instead of
//! jumping. Only while `armed`, so window resizes and scrolling never set
//! things sliding.

use std::time::Instant;

use gpui_kit::*;

use crate::ui::theme::motion;

pub struct Glide {
    id: ElementId,
    child: AnyElement,
    armed: bool,
}

impl Glide {
    pub fn new(id: impl Into<ElementId>, child: impl IntoElement) -> Self {
        Self { id: id.into(), child: child.into_any_element(), armed: false }
    }

    /// Slide on a change of place now (the layout was just rearranged).
    pub fn armed(mut self, armed: bool) -> Self {
        self.armed = armed;
        self
    }
}

#[derive(Clone, Copy)]
struct GlideState {
    /// Where the layout put the element last frame, scrolling aside.
    place: Point<Pixels>,
    x: SpringState,
    y: SpringState,
    at: Instant,
}

const REST: SpringState = SpringState { position: 0.0, velocity: 0.0 };

impl IntoElement for Glide {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Glide {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let place = bounds.origin - window.element_offset();
        let now = Instant::now();
        let armed = self.armed && !cx.reduce_motion();
        let id = id.expect("Glide has an id");
        let offset = window.with_element_state(id, |state: Option<GlideState>, window| {
            let mut s = state.unwrap_or(GlideState { place, x: REST, y: REST, at: now });
            let dt = now.duration_since(s.at).as_secs_f32();
            s.x = motion::GLIDE.step(s.x, 0.0, dt);
            s.y = motion::GLIDE.step(s.y, 0.0, dt);
            if s.place != place {
                if armed {
                    // Keep it where it was on screen; the spring brings it home.
                    s.x.position += f32::from(s.place.x - place.x);
                    s.y.position += f32::from(s.place.y - place.y);
                } else {
                    (s.x, s.y) = (REST, REST);
                }
            }
            let settled = motion::GLIDE.is_settled(s.x, 0.0, 0.5) && motion::GLIDE.is_settled(s.y, 0.0, 0.5);
            if settled {
                (s.x, s.y) = (REST, REST);
            } else {
                window.request_animation_frame();
            }
            s.place = place;
            s.at = now;
            (point(px(s.x.position), px(s.y.position)), s)
        });
        window.with_element_offset(offset, |window| self.child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
