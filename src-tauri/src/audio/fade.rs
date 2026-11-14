use std::time::{Duration, Instant};

pub const FADE_DURATION: Duration = Duration::from_secs(3);
pub const DUCK_RATIO: f32 = 0.15;

pub fn level(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress.clamp(0.0, 1.0)
}

pub trait Volume {
    fn get(&self) -> Result<f32, String>;
    fn set(&self, value: f32) -> Result<(), String>;
    fn overridden(&self) -> bool;
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    ManualOverride,
}

// Comparison is a second guard against notification delivery latency.
pub fn may_write(current: f32, expected: f32, overridden: bool) -> bool {
    !overridden && current.is_finite() && (current - expected).abs() < 0.0001
}

pub fn step(volume: &impl Volume, expected: &mut f32, next: f32) -> Result<Outcome, String> {
    if !may_write(volume.get()?, *expected, volume.overridden()) {
        return Ok(Outcome::ManualOverride);
    }
    volume.set(next)?;
    *expected = next;
    Ok(Outcome::Completed)
}

pub fn fade(
    volume: &impl Volume,
    expected: &mut f32,
    target: f32,
    duration: Duration,
) -> Result<Outcome, String> {
    let start = *expected;
    let began = Instant::now();
    loop {
        let progress = if duration.is_zero() {
            1.0
        } else {
            (began.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.0)
        };
        let outcome = step(volume, expected, level(start, target, progress))?;
        if outcome == Outcome::ManualOverride || progress >= 1.0 {
            return Ok(outcome);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Fake {
        value: Cell<f32>,
        external: bool,
        fail: bool,
        writes: Cell<u32>,
    }
    impl Volume for Fake {
        fn get(&self) -> Result<f32, String> {
            Ok(self.value.get())
        }
        fn set(&self, value: f32) -> Result<(), String> {
            if self.fail {
                return Err("session disconnected".into());
            }
            self.value.set(value);
            self.writes.set(self.writes.get() + 1);
            Ok(())
        }
        fn overridden(&self) -> bool {
            self.external
        }
    }
    fn fake(value: f32) -> Fake {
        Fake {
            value: Cell::new(value),
            external: false,
            fail: false,
            writes: Cell::new(0),
        }
    }
    #[test]
    fn interpolation_and_relative_target() {
        assert_eq!(level(0.8, 0.12, 0.0), 0.8);
        assert!((level(0.8, 0.8 * DUCK_RATIO, 1.0) - 0.12).abs() < 0.00001);
        assert!((level(0.12, 0.8, 1.0) - 0.8).abs() < 0.00001);
        assert_eq!(level(0.0, 0.0, 0.5), 0.0);
        assert_eq!(FADE_DURATION, Duration::from_secs(3));
    }
    #[test]
    fn user_change_is_never_written_over() {
        let volume = fake(0.5);
        assert_eq!(
            step(&volume, &mut 0.12, 0.8).unwrap(),
            Outcome::ManualOverride
        );
        assert_eq!(volume.writes.get(), 0);
        let volume = Fake {
            external: true,
            ..fake(0.12)
        };
        assert_eq!(
            step(&volume, &mut 0.12, 0.8).unwrap(),
            Outcome::ManualOverride
        );
        assert_eq!(volume.writes.get(), 0);
    }
    #[test]
    fn errors_preserve_last_successful_level() {
        let volume = Fake {
            fail: true,
            ..fake(0.8)
        };
        let mut expected = 0.8;
        assert!(step(&volume, &mut expected, 0.12).is_err());
        assert_eq!(expected, 0.8);
    }
    #[test]
    fn complete_duck_and_restore() {
        let volume = fake(0.8);
        let mut expected = 0.8;
        assert_eq!(
            fade(&volume, &mut expected, 0.12, Duration::ZERO).unwrap(),
            Outcome::Completed
        );
        fade(&volume, &mut expected, 0.8, Duration::ZERO).unwrap();
        assert!((volume.value.get() - 0.8).abs() < 0.00001);
    }

    #[test]
    fn manual_change_mid_fade_stops_subsequent_writes() {
        let volume = fake(0.8);
        let mut expected = 0.8;
        step(&volume, &mut expected, 0.6).unwrap();
        volume.value.set(0.4);
        assert_eq!(
            step(&volume, &mut expected, 0.3).unwrap(),
            Outcome::ManualOverride
        );
        assert_eq!(
            fade(&volume, &mut expected, 0.8, Duration::ZERO).unwrap(),
            Outcome::ManualOverride
        );
        assert_eq!(volume.value.get(), 0.4);
        assert_eq!(volume.writes.get(), 1);
    }

    #[test]
    fn fade_levels_are_monotonic_and_bounded() {
        for original in [0.0, 0.01, 0.5, 1.0] {
            let target = original * DUCK_RATIO;
            let mut previous = original;
            for tick in 0..=150 {
                let next = level(original, target, tick as f32 / 150.0);
                assert!(next >= target - 0.000001 && next <= previous + 0.000001);
                previous = next;
            }
        }
        assert!(!may_write(f32::NAN, 0.5, false));
    }
}
