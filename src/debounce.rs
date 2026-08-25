use std::time::{Duration, Instant};

use crate::config::AntiDoubleClickConfig;

/// Per-button rising-edge cooldown (standard double-click / bounce filter).
///
/// A press is accepted only if that same button has not been accepted within
/// `window_ms`. Releases always pass through immediately, so a held button is
/// never locked and a human mash whose interval is longer than the window is
/// not dropped.
#[derive(Debug, Default, Clone)]
pub struct ButtonDebouncer {
    last_physical: u16,
    suppressed: u16,
    last_accepted_press: [Option<Instant>; 16],
}

impl ButtonDebouncer {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn apply(&mut self, buttons: u16, cfg: &AntiDoubleClickConfig, now: Instant) -> u16 {
        if !cfg.enabled || cfg.buttons == 0 || cfg.window_ms == 0 {
            self.last_physical = buttons;
            self.suppressed = 0;
            return buttons;
        }

        let window = Duration::from_millis(u64::from(cfg.window_ms));
        let mask = cfg.buttons;
        let rising = buttons & !self.last_physical;
        let falling = self.last_physical & !buttons;

        self.suppressed &= !falling;
        self.suppressed &= mask;

        let mut remaining = rising & mask;
        while remaining != 0 {
            let bit = remaining & remaining.wrapping_neg();
            remaining ^= bit;
            let i = bit.trailing_zeros() as usize;

            let too_soon = self.last_accepted_press[i]
                .is_some_and(|t| now.saturating_duration_since(t) < window);

            if too_soon {
                self.suppressed |= bit;
            } else {
                self.last_accepted_press[i] = Some(now);
            }
        }

        self.last_physical = buttons;
        buttons & !(self.suppressed & mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BTN_A, BTN_DPAD_DOWN, BTN_DPAD_UP};

    fn cfg(window_ms: u16, buttons: u16) -> AntiDoubleClickConfig {
        AntiDoubleClickConfig {
            enabled: true,
            window_ms,
            buttons,
        }
    }

    fn t0() -> Instant {
        Instant::now()
    }

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn ghost_press_inside_window_is_dropped() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(d.apply(0, &filter, at(start, 10)), 0);
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 20)), 0);
        assert_eq!(d.apply(0, &filter, at(start, 30)), 0);
    }

    #[test]
    fn press_after_window_is_accepted() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(d.apply(0, &filter, at(start, 10)), 0);
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 60)), BTN_DPAD_UP);
    }

    #[test]
    fn mash_with_interleaved_ghosts_keeps_real_presses() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        for i in 0..4 {
            let t = i * 80;
            assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, t)), BTN_DPAD_UP);
            assert_eq!(d.apply(0, &filter, at(start, t + 8)), 0);
            // Hardware ghost ~15ms after the real tap.
            assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, t + 15)), 0);
            assert_eq!(d.apply(0, &filter, at(start, t + 22)), 0);
        }
    }

    #[test]
    fn hold_is_not_treated_as_a_new_press() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 8)), BTN_DPAD_UP);
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 16)), BTN_DPAD_UP);
    }

    #[test]
    fn suppressed_press_stays_down_until_physical_release() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(d.apply(0, &filter, at(start, 5)), 0);
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 15)), 0);
        // Still physically held inside the window: keep reporting released.
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 25)), 0);
        assert_eq!(d.apply(0, &filter, at(start, 40)), 0);
    }

    #[test]
    fn other_buttons_are_unaffected() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(
            d.apply(BTN_DPAD_UP | BTN_DPAD_DOWN, &filter, at(start, 10)),
            BTN_DPAD_UP | BTN_DPAD_DOWN
        );
        assert_eq!(d.apply(BTN_A, &filter, at(start, 15)), BTN_A);
    }

    #[test]
    fn unmasked_button_is_never_filtered() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP);

        assert_eq!(d.apply(BTN_A, &filter, start), BTN_A);
        assert_eq!(d.apply(0, &filter, at(start, 5)), 0);
        assert_eq!(d.apply(BTN_A, &filter, at(start, 10)), BTN_A);
    }

    #[test]
    fn disabled_is_passthrough() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = AntiDoubleClickConfig {
            enabled: false,
            window_ms: 50,
            buttons: BTN_DPAD_UP,
        };

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(d.apply(0, &filter, at(start, 5)), 0);
        assert_eq!(d.apply(BTN_DPAD_UP, &filter, at(start, 10)), BTN_DPAD_UP);
    }

    #[test]
    fn different_direction_is_not_blocked() {
        let mut d = ButtonDebouncer::default();
        let start = t0();
        let filter = cfg(50, BTN_DPAD_UP | BTN_DPAD_DOWN);

        assert_eq!(d.apply(BTN_DPAD_UP, &filter, start), BTN_DPAD_UP);
        assert_eq!(d.apply(0, &filter, at(start, 8)), 0);
        assert_eq!(
            d.apply(BTN_DPAD_DOWN, &filter, at(start, 16)),
            BTN_DPAD_DOWN
        );
    }
}
