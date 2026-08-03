/// Reduces a DOM `MouseEvent.buttons` bitmask to the `MouseEvent.button` index
/// of the highest-priority button still held, or `None` when none remain.
///
/// `buttons` reports every button currently down while GPUI tracks a single
/// pressed button, so on a chord release (where only the released bit is
/// cleared) this recovers the button that is still held instead of clearing the
/// state outright. The pairs translate the `buttons` bit layout (1 primary, 2
/// secondary, 4 auxiliary, 8 back, 16 forward) to the `button` index layout (0
/// left, 1 middle, 2 right, 3 back, 4 forward); unknown bits map to `None`.
pub(super) fn dom_button_from_buttons(buttons: u16) -> Option<i16> {
    [(1, 0), (4, 1), (2, 2), (8, 3), (16, 4)]
        .into_iter()
        .find_map(|(mask, button)| (buttons & mask != 0).then_some(button))
}

/// Whether a `pointerdown`/`pointerup` should be handled on the pointer path.
///
/// Mouse input is routed through the per-button `mousedown`/`mouseup` listeners
/// instead: a pointer event collapses simultaneous buttons into one
/// active/inactive transition and drops a chord's intermediate release. Touch
/// and pen have no per-button events, so they stay on the pointer path.
pub(super) fn should_handle_pointer_button_event(pointer_type: &str) -> bool {
    pointer_type != "mouse"
}

/// How long after a touch or pen contact a bare mouse edge is still read as
/// the browser's synthesized compatibility event rather than a real mouse.
/// WebKit delivers its pair immediately after `touchend`; the window is padded
/// for slow devices. A hybrid-device user who taps and then clicks a mouse at
/// the same spot inside this window loses that click, the same trade every
/// ghost-click suppressor since FastClick has made.
pub(super) const GHOST_MOUSE_WINDOW_MS: f64 = 1500.0;

/// How far a compatibility mouse edge may land from the recorded contact and
/// still be treated as its echo. WebKit synthesizes at the exact lift point,
/// so this only needs to absorb coordinate rounding, but a fingertip-scale
/// radius also keeps a drifted recording from letting the echo through.
pub(super) const GHOST_MOUSE_RADIUS_PX: f32 = 32.0;

/// Whether a `mousedown`/`mouseup` is the compatibility echo of a recent touch
/// or pen contact, and must be swallowed instead of dispatched.
///
/// Per the Pointer Events spec, `preventDefault()` on `pointerdown` suppresses
/// these compatibility events, and Chromium honors that. iOS Safari does not:
/// after `touchend` it synthesizes a `mousedown`/`mouseup` pair at the touch
/// point regardless, so every tap would dispatch twice — a toggle control
/// activates and immediately un-activates. `last_contact` is the most recent
/// non-mouse pointer edge as `(x, y, time_ms)`; `now` is injected so the
/// classification is a pure function of its inputs.
pub(super) fn is_ghost_mouse_edge(
    last_contact: Option<(f32, f32, f64)>,
    x: f32,
    y: f32,
    now: f64,
) -> bool {
    let Some((contact_x, contact_y, contact_time)) = last_contact else {
        return false;
    };
    if now - contact_time > GHOST_MOUSE_WINDOW_MS {
        return false;
    }
    let distance = ((x - contact_x).powi(2) + (y - contact_y).powi(2)).sqrt();
    distance <= GHOST_MOUSE_RADIUS_PX
}

#[cfg(test)]
mod tests {
    use super::{
        GHOST_MOUSE_RADIUS_PX, GHOST_MOUSE_WINDOW_MS, dom_button_from_buttons,
        is_ghost_mouse_edge, should_handle_pointer_button_event,
    };

    #[test]
    fn remaining_mouse_buttons_are_classified_over_complete_representative_sets() {
        let cases = [
            (0, None),
            (1, Some(0)),
            (2, Some(2)),
            (3, Some(0)),
            (4, Some(1)),
            (5, Some(0)),
            (6, Some(1)),
            (7, Some(0)),
            (8, Some(3)),
            (16, Some(4)),
            (24, Some(3)),
            (32, None),
            (33, Some(0)),
        ];

        for (buttons, expected) in cases {
            assert_eq!(
                dom_button_from_buttons(buttons),
                expected,
                "buttons={buttons}"
            );
        }
    }

    #[test]
    fn mouse_pointer_edges_defer_to_per_button_mouse_events() {
        for (pointer_type, expected) in
            [("mouse", false), ("touch", true), ("pen", true), ("", true)]
        {
            assert_eq!(
                should_handle_pointer_button_event(pointer_type),
                expected,
                "pointer_type={pointer_type:?}",
            );
        }
    }

    #[test]
    fn ghost_mouse_edges_are_classified_over_position_and_time_sets() {
        let contact = Some((100.0, 200.0, 10_000.0));
        let cases = [
            // No touch has ever happened: every mouse edge is real.
            (None, 100.0, 200.0, 10_001.0, false),
            // Immediate echo at the exact lift point.
            (contact, 100.0, 200.0, 10_001.0, true),
            // Echo offset by coordinate rounding.
            (contact, 101.0, 199.0, 10_050.0, true),
            // On the radius boundary: still an echo.
            (contact, 100.0 + GHOST_MOUSE_RADIUS_PX, 200.0, 10_050.0, true),
            // Just past the radius: a real mouse somewhere else.
            (contact, 100.0 + GHOST_MOUSE_RADIUS_PX + 1.0, 200.0, 10_050.0, false),
            // On the window boundary: still an echo.
            (contact, 100.0, 200.0, 10_000.0 + GHOST_MOUSE_WINDOW_MS, true),
            // Just past the window: the touch is stale, the mouse is real.
            (contact, 100.0, 200.0, 10_001.0 + GHOST_MOUSE_WINDOW_MS, false),
            // Far away AND late: unambiguously real.
            (contact, 500.0, 700.0, 20_000.0, false),
        ];

        for (index, (last_contact, x, y, now, expected)) in cases.into_iter().enumerate() {
            assert_eq!(
                is_ghost_mouse_edge(last_contact, x, y, now),
                expected,
                "case {index}: last_contact={last_contact:?} x={x} y={y} now={now}"
            );
        }
    }
}
