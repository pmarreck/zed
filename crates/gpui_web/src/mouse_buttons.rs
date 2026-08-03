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

/// The device class of the most recent pointer activity. The mouse listeners
/// consult it to tell a real mouse edge from a compatibility echo of a touch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PointerDevice {
    Mouse,
    TouchOrPen,
}

/// Advances the ghost-mouse guard's state from a DOM `pointerType`.
///
/// Per the Pointer Events spec, `preventDefault()` on `pointerdown` suppresses
/// compatibility mouse events, and Chromium honors that. iOS Safari does not:
/// after `touchend` it synthesizes a mousedown/mouseup pair at the touch point
/// regardless, so every tap would dispatch twice - a toggle control activates
/// and immediately un-activates. The discriminator is structural, not timed:
/// every edge a real mouse produces is preceded by its own pointer twin (a
/// `pointerdown`, or a `pointermove` for a chord's collapsed second button),
/// while a synthesized compatibility event is a bare `MouseEvent` with no
/// pointer twin at all. So whenever the latest pointer activity was touch or
/// pen, a bare mouse edge can only be an echo and is swallowed.
pub(super) fn pointer_device_from_type(pointer_type: &str) -> PointerDevice {
    if pointer_type == "mouse" {
        PointerDevice::Mouse
    } else {
        PointerDevice::TouchOrPen
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PointerDevice, dom_button_from_buttons, pointer_device_from_type,
        should_handle_pointer_button_event,
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
    fn pointer_devices_are_classified_over_the_pointer_type_domain() {
        // The guard swallows bare mouse edges whenever the latest pointer
        // activity was not a mouse, so an unknown or empty pointerType must
        // land on the swallowing side: browsers only synthesize compatibility
        // mouse events for non-mouse pointers, never the reverse.
        for (pointer_type, expected) in [
            ("mouse", PointerDevice::Mouse),
            ("touch", PointerDevice::TouchOrPen),
            ("pen", PointerDevice::TouchOrPen),
            ("", PointerDevice::TouchOrPen),
            ("unknown-future-device", PointerDevice::TouchOrPen),
        ] {
            assert_eq!(
                pointer_device_from_type(pointer_type),
                expected,
                "pointer_type={pointer_type:?}"
            );
        }
    }
}
