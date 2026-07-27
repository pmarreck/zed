//! Which DOM element owns focus, and why it is not the editable one by default.
//!
//! A touch platform presents its software keyboard when an *editable* element
//! holds focus; the tap is only the gesture that lets it present. Focusing the
//! hidden `<input>` at window creation therefore makes every tap summon the iOS
//! keyboard, even for an application that declares no text control. Counting
//! `focus()` calls cannot observe that, because the offending call happens once
//! at startup and the element simply keeps focus afterwards.
//!
//! Focus instead follows GPUI's existing text/IME lifecycle: `set_input_handler`
//! means a real text control became active, `take_input_handler` means it went
//! away. Keeping the decision a pure function lets the invariant be checked
//! without a browser, over the complete input domain.

/// The element that must own DOM focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusHost {
    /// The canvas, made focusable with `tabindex="-1"`. It is not editable, so
    /// it delivers hardware keys without summoning a software keyboard.
    Canvas,
    /// The hidden editable `<input>`. Required for text and IME composition,
    /// and precisely what makes the software keyboard appear, so it may hold
    /// focus only while a text input handler is actually active.
    TextInput,
}

/// Maps "does the application currently have an active GPUI text/IME input
/// handler?" onto the element that must own DOM focus.
///
/// This is the whole software-keyboard policy: the editable element is
/// reachable only through a genuine text-entry request, never through ordinary
/// pointer interaction or window startup.
pub(super) fn focus_host(text_input_active: bool) -> FocusHost {
    if text_input_active {
        FocusHost::TextInput
    } else {
        FocusHost::Canvas
    }
}

/// Which of this window's elements currently owns DOM focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusOwner {
    TextInput,
    Canvas,
    /// Neither — `document.body` at startup, or an element outside this window.
    Neither,
}

/// Returns the focus move required to satisfy the policy, or `None` when the
/// DOM already satisfies it.
///
/// Idempotence is load-bearing, not a micro-optimization. GPUI installs and
/// removes its input handler as part of ordinary rendering, so applying focus
/// unconditionally calls `focus()`/`blur()` every frame. Each call fires
/// synchronous DOM focus events, whose listeners re-enter GPUI and provoke
/// another render — an unbounded re-entrant loop that panics the runtime.
pub(super) fn focus_transition(text_input_active: bool, owner: FocusOwner) -> Option<FocusHost> {
    // Nomination stays the single source of truth; this only adds "already
    // there, so do not touch the DOM".
    let nominated = focus_host(text_input_active);
    let satisfied = matches!(
        (nominated, owner),
        (FocusHost::TextInput, FocusOwner::TextInput) | (FocusHost::Canvas, FocusOwner::Canvas)
    );
    if satisfied { None } else { Some(nominated) }
}

#[cfg(test)]
mod tests {
    use super::{FocusHost, FocusOwner, focus_host, focus_transition};

    /// 2 x 3 is a finite domain, so this exhausts it rather than sampling it.
    #[test]
    fn focus_transition_is_exhaustively_classified_over_the_complete_domain() {
        let cases = [
            (false, FocusOwner::Neither, Some(FocusHost::Canvas)),
            (false, FocusOwner::Canvas, None),
            (false, FocusOwner::TextInput, Some(FocusHost::Canvas)),
            (true, FocusOwner::Neither, Some(FocusHost::TextInput)),
            (true, FocusOwner::Canvas, Some(FocusHost::TextInput)),
            (true, FocusOwner::TextInput, None),
        ];
        for (text_input_active, owner, expected) in cases {
            assert_eq!(
                focus_transition(text_input_active, owner),
                expected,
                "active={text_input_active} owner={owner:?}",
            );
        }
    }

    /// The property that keeps the runtime alive: once the DOM already agrees
    /// with the policy, applying it again must touch nothing. Without this,
    /// every rendered frame re-enters GPUI through a synchronous focus event.
    #[test]
    fn a_satisfied_policy_requests_no_dom_focus_change() {
        assert_eq!(focus_transition(true, FocusOwner::TextInput), None);
        assert_eq!(focus_transition(false, FocusOwner::Canvas), None);
    }

    /// The complementary half: an unsatisfied policy must still act, or focus
    /// would never reach the text input and text entry would be dead.
    #[test]
    fn an_unsatisfied_policy_always_requests_the_nominated_host() {
        for owner in [FocusOwner::Neither, FocusOwner::Canvas] {
            assert_eq!(
                focus_transition(true, owner),
                Some(FocusHost::TextInput),
                "{owner:?}"
            );
        }
        for owner in [FocusOwner::Neither, FocusOwner::TextInput] {
            assert_eq!(
                focus_transition(false, owner),
                Some(FocusHost::Canvas),
                "{owner:?}"
            );
        }
    }
}

#[cfg(test)]
mod host_tests {
    use super::{FocusHost, focus_host};

    /// `bool` is a finite domain, so this exhausts it rather than sampling it.
    #[test]
    fn focus_host_is_exhaustively_classified_over_the_complete_domain() {
        let cases = [(false, FocusHost::Canvas), (true, FocusHost::TextInput)];
        for (text_input_active, expected) in cases {
            assert_eq!(
                focus_host(text_input_active),
                expected,
                "text_input_active={text_input_active} must map to {expected:?}",
            );
        }
    }

    /// The property that actually protects Peter's iPhone: with no active text
    /// handler, nothing editable may be nominated for focus.
    #[test]
    fn no_editable_host_is_nominated_without_an_active_text_handler() {
        assert_ne!(focus_host(false), FocusHost::TextInput);
    }

    /// The complementary half. A fix that never focuses the editable element
    /// would pass the test above while silently breaking all text entry.
    #[test]
    fn an_active_text_handler_nominates_the_editable_host_so_the_keyboard_appears() {
        assert_eq!(focus_host(true), FocusHost::TextInput);
    }
}
