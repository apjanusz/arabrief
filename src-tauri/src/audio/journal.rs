//! Durable write-ahead snapshot. Only the audio worker owns session interfaces.
use super::fade::{self, Outcome, Volume};
use crate::settings::SettingsRepository;
use rusqlite::{OptionalExtension, params};
use std::{cell::Cell, sync::Arc};

#[derive(Clone, Debug)]
pub(super) struct Snapshot {
    pub session_id: String,
    pub original: f32,
    pub expected: f32,
    pub pending: f32,
}
impl Snapshot {
    fn validate(&self) -> Result<(), String> {
        if self.session_id.is_empty()
            || self.session_id.len() > 4096
            || [self.original, self.expected, self.pending]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Nieprawidłowy zapis odzyskiwania audio".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct Journal(pub Arc<SettingsRepository>);
impl Journal {
    pub fn load(&self) -> Result<Option<Snapshot>, String> {
        self.0.with_database(|db| {
            let snapshot = db
                .query_row(
                    "SELECT session_id,original,expected,pending FROM audio_recovery WHERE id=1",
                    [],
                    |r| {
                        Ok(Snapshot {
                            session_id: r.get(0)?,
                            original: r.get(1)?,
                            expected: r.get(2)?,
                            pending: r.get(3)?,
                        })
                    },
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some(snapshot) = &snapshot {
                snapshot.validate()?;
            }
            Ok(snapshot)
        })
    }
    fn store(&self, snapshot: &Snapshot) -> Result<(), String> {
        snapshot.validate()?;
        self.0.with_database(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO audio_recovery VALUES(1,?1,?2,?3,?4)",
                params![
                    snapshot.session_id,
                    snapshot.original,
                    snapshot.expected,
                    snapshot.pending
                ],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())
        })
    }
    pub fn clear(&self) -> Result<(), String> {
        self.0.with_database(|db| {
            db.execute("DELETE FROM audio_recovery WHERE id=1", [])
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
    }
}

/// Persist both possible levels before the COM call; a crash can happen on
/// either side of that call. A different level is treated as a user override.
pub(super) struct JournaledVolume<V: Volume> {
    pub inner: V,
    journal: Journal,
    session_id: String,
    original: f32,
    expected: Cell<f32>,
}
impl<V: Volume> JournaledVolume<V> {
    pub fn new(
        inner: V,
        journal: Journal,
        session_id: String,
        original: f32,
    ) -> Result<Self, String> {
        journal.store(&Snapshot {
            session_id: session_id.clone(),
            original,
            expected: original,
            pending: original,
        })?;
        Ok(Self {
            inner,
            journal,
            session_id,
            original,
            expected: Cell::new(original),
        })
    }
}
impl<V: Volume> Volume for JournaledVolume<V> {
    fn get(&self) -> Result<f32, String> {
        self.inner.get()
    }
    fn overridden(&self) -> bool {
        self.inner.overridden()
    }
    fn set(&self, value: f32) -> Result<(), String> {
        self.journal.store(&Snapshot {
            session_id: self.session_id.clone(),
            original: self.original,
            expected: self.expected.get(),
            pending: value,
        })?;
        // SQLite sync can take time; recheck user changes after the durable write.
        if !fade::may_write(
            self.inner.get()?,
            self.expected.get(),
            self.inner.overridden(),
        ) {
            return Err("Głośność sesji została zmieniona podczas zapisu odzyskiwania".into());
        }
        self.inner.set(value)?;
        self.expected.set(value);
        Ok(())
    }
}

pub(super) fn recover(
    volume: impl Volume,
    journal: &Journal,
    snapshot: &Snapshot,
) -> Result<Outcome, String> {
    let current = volume.get()?;
    if !fade::may_write(current, snapshot.expected, volume.overridden())
        && !fade::may_write(current, snapshot.pending, volume.overridden())
    {
        journal.clear()?;
        return Ok(Outcome::ManualOverride);
    }
    // A single idempotent write, no fade on startup. Retain the original snapshot
    // on failure, including a crash between the COM call and clearing the row.
    journal.store(&Snapshot {
        expected: current,
        pending: snapshot.original,
        ..snapshot.clone()
    })?;
    if !fade::may_write(volume.get()?, current, volume.overridden()) {
        journal.clear()?;
        return Ok(Outcome::ManualOverride);
    }
    volume.set(snapshot.original)?;
    journal.clear()?;
    Ok(Outcome::Completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        value: Cell<f32>,
        fail: bool,
    }
    impl Volume for &Fake {
        fn get(&self) -> Result<f32, String> {
            Ok(self.value.get())
        }
        fn set(&self, v: f32) -> Result<(), String> {
            if self.fail {
                return Err("disconnected".into());
            }
            self.value.set(v);
            Ok(())
        }
        fn overridden(&self) -> bool {
            false
        }
    }
    fn journal() -> Journal {
        Journal(Arc::new(
            SettingsRepository::open(std::path::Path::new(":memory:")).unwrap(),
        ))
    }
    fn snapshot() -> Snapshot {
        Snapshot {
            session_id: "endpoint|instance".into(),
            original: 0.8,
            expected: 0.5,
            pending: 0.12,
        }
    }
    #[test]
    fn recovers_crash_on_either_side_of_audio_write_and_is_idempotent() {
        for value in [0.5, 0.12] {
            let j = journal();
            let s = snapshot();
            j.store(&s).unwrap();
            let v = Fake {
                value: Cell::new(value),
                fail: false,
            };
            assert_eq!(recover(&v, &j, &s).unwrap(), Outcome::Completed);
            assert_eq!(v.value.get(), 0.8);
            assert!(j.load().unwrap().is_none());
        }
    }
    #[test]
    fn manual_override_is_preserved() {
        let j = journal();
        let s = snapshot();
        j.store(&s).unwrap();
        let v = Fake {
            value: Cell::new(0.3),
            fail: false,
        };
        assert_eq!(recover(&v, &j, &s).unwrap(), Outcome::ManualOverride);
        assert_eq!(v.value.get(), 0.3);
        assert!(j.load().unwrap().is_none());
    }
    #[test]
    fn failed_restore_keeps_snapshot_for_retry() {
        let j = journal();
        let s = snapshot();
        j.store(&s).unwrap();
        let v = Fake {
            value: Cell::new(0.12),
            fail: true,
        };
        assert!(recover(&v, &j, &s).is_err());
        let retained = j.load().unwrap().unwrap();
        assert_eq!(retained.original, 0.8);
        assert_eq!(retained.expected, 0.12);
        assert_eq!(retained.pending, 0.8);
        let v = Fake {
            value: Cell::new(0.12),
            fail: false,
        };
        recover(&v, &j, &retained).unwrap();
        assert_eq!(v.value.get(), 0.8);
    }
    #[test]
    fn journal_failure_prevents_audio_write() {
        let j = journal();
        let v = Fake {
            value: Cell::new(0.8),
            fail: false,
        };
        let wrapped = JournaledVolume::new(&v, j.clone(), "session".into(), 0.8).unwrap();
        j.0.with_database(|db| {
            db.execute("DROP TABLE audio_recovery", []).unwrap();
            Ok(())
        })
        .unwrap();
        assert!(wrapped.set(0.12).is_err());
        assert_eq!(v.value.get(), 0.8);
    }
    #[test]
    fn cancelled_cycle_can_restore_and_clear_before_next_cycle() {
        let j = journal();
        let v = Fake {
            value: Cell::new(0.8),
            fail: false,
        };
        let wrapped = JournaledVolume::new(&v, j.clone(), "session".into(), 0.8).unwrap();
        wrapped.set(0.4).unwrap();
        let s = j.load().unwrap().unwrap();
        recover(&v, &j, &s).unwrap();
        assert_eq!(v.value.get(), 0.8);
        assert!(j.load().unwrap().is_none());
    }
    #[test]
    fn interrupted_write_survives_database_reopen_and_a_second_recovery_crash() {
        let path = std::env::temp_dir().join(format!(
            "arabrief-audio-recovery-{}-{}.sqlite",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let j = Journal(Arc::new(SettingsRepository::open(&path).unwrap()));
            j.store(&snapshot()).unwrap();
        }
        {
            let j = Journal(Arc::new(SettingsRepository::open(&path).unwrap()));
            let s = j.load().unwrap().unwrap();
            assert_eq!(s.session_id, "endpoint|instance");
            // Simulate recovery reaching Windows, then dying before journal.clear().
            j.store(&Snapshot {
                expected: 0.12,
                pending: s.original,
                ..s
            })
            .unwrap();
        }
        {
            let j = Journal(Arc::new(SettingsRepository::open(&path).unwrap()));
            let s = j.load().unwrap().unwrap();
            let v = Fake {
                value: Cell::new(0.8),
                fail: false,
            };
            assert_eq!(recover(&v, &j, &s).unwrap(), Outcome::Completed);
            assert_eq!(v.value.get(), 0.8);
            assert!(j.load().unwrap().is_none());
        }
        std::fs::remove_file(path).unwrap();
    }
}
