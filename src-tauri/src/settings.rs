use rusqlite::{Connection, TransactionBehavior, params};
use std::{path::Path, sync::Mutex, time::Duration};

fn default_ui_language() -> String {
    "auto".into()
}

#[tauri::command]
pub fn system_ui_language() -> &'static str {
    #[cfg(windows)]
    {
        // PRIMARYLANGID: Polish is 0x15, regardless of regional sublanguage.
        let language = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        if language & 0x03ff == 0x15 {
            return "pl";
        }
    }
    "en"
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferredAudioSource {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    #[serde(default = "default_ui_language")]
    pub ui_language: String,
    pub version: u32,
    pub preferred_audio_source: Option<PreferredAudioSource>,
    pub interval_ms: u32,
    pub duck_volume: f32,
    pub fade_out_ms: u32,
    pub fade_in_ms: u32,
    pub narrator_volume: f32,
    pub broadcast_min_duration_ms: u32,
    pub broadcast_max_duration_ms: u32,
    pub topics: Vec<String>,
    pub open_ai_voice: String,
    pub text_model: String,
    pub auto_start: bool,
    pub start_scheduler_on_launch: bool,
    pub language: String,
    pub news_source_urls: Vec<String>,
}
impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        fn text(s: &str, max: usize) -> bool {
            !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
        }
        if self.version != 1
            || !["auto", "pl", "en"].contains(&self.ui_language.as_str())
            || !(60000..=86400000).contains(&self.interval_ms)
            || self.fade_out_ms > 30000
            || self.fade_in_ms > 30000
            || !self.duck_volume.is_finite()
            || !(0.0..=1.0).contains(&self.duck_volume)
            || !self.narrator_volume.is_finite()
            || !(0.0..=1.0).contains(&self.narrator_volume)
            || self.broadcast_min_duration_ms < 30000
            || self.broadcast_max_duration_ms > 900000
            || self.broadcast_min_duration_ms > self.broadcast_max_duration_ms
            || !text(&self.open_ai_voice, 64)
            || !text(&self.text_model, 100)
            || !self
                .text_model
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
            || !self
                .open_ai_voice
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !text(&self.language, 32)
            || self
                .preferred_audio_source
                .as_ref()
                .is_some_and(|s| !text(&s.id, 4096) || !text(&s.name, 256))
            || self.topics.len() > 30
            || self.topics.iter().any(|s| !text(s, 160))
            || self.news_source_urls.len() > 50
            || self
                .news_source_urls
                .iter()
                .any(|s| !text(s, 2048) || !(s.starts_with("https://") || s.starts_with("http://")))
            || self
                .topics
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.topics.len()
            || self
                .news_source_urls
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.news_source_urls.len()
        {
            return Err("Nieprawidłowe ustawienia AraBrief".into());
        }
        Ok(())
    }
}

pub struct SettingsRepository(Mutex<Connection>);
impl SettingsRepository {
    pub(crate) fn with_database<T>(
        &self,
        action: impl FnOnce(&mut Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut db = self.0.lock().map_err(|_| "Baza danych niedostępna")?;
        action(&mut db)
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut db = Connection::open(path).map_err(|e| e.to_string())?;
        db.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| e.to_string())?;
        db.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| e.to_string())?;
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        migrate(&mut db).map_err(|e| e.to_string())?;
        let repo = Self(Mutex::new(db));
        repo.load()?; // Do not overwrite corrupt or unsupported data with defaults.
        Ok(repo)
    }
    pub fn load(&self) -> Result<AppSettings, String> {
        let db = self
            .0
            .lock()
            .map_err(|_| "Repozytorium ustawień niedostępne")?;
        let mut settings = db.query_row("SELECT preferred_audio_id, preferred_audio_name, interval_ms, duck_volume, fade_out_ms, fade_in_ms, narrator_volume, broadcast_min_ms, broadcast_max_ms, openai_voice, auto_start, language FROM settings WHERE id=1", [], |row| {
            let id: Option<String> = row.get(0)?;
            let name: Option<String> = row.get(1)?;
            Ok(AppSettings {
                ui_language: default_ui_language(),
                version: 1, start_scheduler_on_launch: false, preferred_audio_source: id.zip(name).map(|(id,name)| PreferredAudioSource { id, name }),
                interval_ms: row.get(2)?, duck_volume: row.get(3)?, fade_out_ms: row.get(4)?, fade_in_ms: row.get(5)?, narrator_volume: row.get(6)?,
                broadcast_min_duration_ms: row.get(7)?, broadcast_max_duration_ms: row.get(8)?, open_ai_voice: row.get(9)?, text_model: String::new(), auto_start: row.get(10)?, language: row.get(11)?, topics: vec![], news_source_urls: vec![],
            })
        }).map_err(|e| e.to_string())?;
        settings.topics = read_list(&db, "SELECT topic FROM settings_topics ORDER BY position")?;
        settings.ui_language = db
            .query_row("SELECT ui_language FROM settings WHERE id=1", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;
        settings.start_scheduler_on_launch = db
            .query_row(
                "SELECT start_scheduler_on_launch FROM settings WHERE id=1",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        settings.text_model = db
            .query_row("SELECT text_model FROM settings WHERE id=1", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        settings.news_source_urls = read_list(
            &db,
            "SELECT url FROM settings_news_sources ORDER BY position",
        )?;
        if !["marin", "cedar"].contains(&settings.open_ai_voice.as_str()) {
            settings.open_ai_voice = "marin".into();
        }
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let mut normalized = settings.clone();
        if !["marin", "cedar"].contains(&normalized.open_ai_voice.as_str()) {
            normalized.open_ai_voice = "marin".into();
        }
        let settings = &normalized;
        settings.validate()?;
        let mut db = self
            .0
            .lock()
            .map_err(|_| "Repozytorium ustawień niedostępne")?;
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let source = settings.preferred_audio_source.as_ref();
        tx.execute(
            "UPDATE settings SET ui_language=?1 WHERE id=1",
            [&settings.ui_language],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE settings SET start_scheduler_on_launch=?1 WHERE id=1",
            [settings.start_scheduler_on_launch],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE settings SET text_model=?1 WHERE id=1",
            [&settings.text_model],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("UPDATE settings SET preferred_audio_id=?1, preferred_audio_name=?2, interval_ms=?3, duck_volume=?4, fade_out_ms=?5, fade_in_ms=?6, narrator_volume=?7, broadcast_min_ms=?8, broadcast_max_ms=?9, openai_voice=?10, auto_start=?11, language=?12 WHERE id=1",
            params![source.map(|s| &s.id), source.map(|s| &s.name), settings.interval_ms, settings.duck_volume, settings.fade_out_ms, settings.fade_in_ms, settings.narrator_volume, settings.broadcast_min_duration_ms, settings.broadcast_max_duration_ms, settings.open_ai_voice, settings.auto_start, settings.language]).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM settings_topics", [])
            .map_err(|e| e.to_string())?;
        for (index, topic) in settings.topics.iter().enumerate() {
            tx.execute(
                "INSERT INTO settings_topics VALUES (?1, ?2)",
                params![topic, index as u32],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute("DELETE FROM settings_news_sources", [])
            .map_err(|e| e.to_string())?;
        for (index, url) in settings.news_source_urls.iter().enumerate() {
            tx.execute(
                "INSERT INTO settings_news_sources VALUES (?1, ?2)",
                params![url, index as u32],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
}
fn read_list(db: &Connection, sql: &str) -> Result<Vec<String>, String> {
    db.prepare(sql)
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())
}
fn migrate(db: &mut Connection) -> rusqlite::Result<()> {
    // Preserve the released schema history even when optional article features are reverted.
    let migrations = [
        include_str!("../migrations/001_settings.sql"),
        include_str!("../migrations/002_news_preferences.sql"),
        include_str!("../migrations/003_news_history.sql"),
        include_str!("../migrations/004_content_generation.sql"),
        include_str!("../migrations/005_speech_assets.sql"),
        include_str!("../migrations/006_article_materials.sql"),
        include_str!("../migrations/007_article_history.sql"),
        include_str!("../migrations/008_pipeline_recovery.sql"),
        include_str!("../migrations/009_background.sql"),
        include_str!("../migrations/010_article_queue.sql"),
        include_str!("../migrations/011_tts_voices.sql"),
        include_str!("../migrations/012_article_web_budget.sql"),
        include_str!("../migrations/013_native_article_recovery.sql"),
        include_str!("../migrations/014_ui_language.sql"),
    ];
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > migrations.len() as u32 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "Baza ma wersję {version}, aplikacja obsługuje maksymalnie {}. Uruchom zgodną wersję aplikacji; dane pozostawiono bez zmian.",
            migrations.len()
        )));
    }
    for (index, sql) in migrations.iter().enumerate() {
        if version < (index + 1) as u32 {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (index + 1) as u32)?;
        }
    }
    tx.commit()
}

#[tauri::command]
pub async fn settings_load(
    app: tauri::AppHandle,
    repo: tauri::State<'_, std::sync::Arc<SettingsRepository>>,
) -> Result<AppSettings, String> {
    let repo = repo.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = repo.load()?;
        settings.auto_start = crate::background::autostart_enabled(&app)?;
        Ok(settings)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn settings_save(
    app: tauri::AppHandle,
    settings: AppSettings,
    repo: tauri::State<'_, std::sync::Arc<SettingsRepository>>,
    audio: tauri::State<'_, crate::audio::DuckingController>,
    player: tauri::State<'_, crate::player::AudioPlayer>,
) -> Result<(), String> {
    let repo = repo.inner().clone();
    let audio = audio.inner().clone();
    let player = player.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        static SAVE_PIPELINE: Mutex<()> = Mutex::new(());
        let _save_guard = SAVE_PIPELINE
            .lock()
            .map_err(|_| "Zapis ustawień niedostępny".to_string())?;
        settings.validate()?;
        let previous_autostart = crate::background::autostart_enabled(&app)?;
        crate::background::set_autostart(&app, settings.auto_start)?;
        if let Err(error) = repo.save(&settings) {
            if let Err(rollback) = crate::background::set_autostart(&app, previous_autostart) {
                return Err(format!(
                    "{error}. Nie udało się cofnąć autostartu: {rollback}"
                ));
            }
            return Err(error);
        }
        apply(&settings, &audio, &player);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn apply(
    settings: &AppSettings,
    audio: &crate::audio::DuckingController,
    player: &crate::player::AudioPlayer,
) {
    audio.configure(
        settings.duck_volume,
        settings.fade_out_ms,
        settings.fade_in_ms,
    );
    let _ = player.set_volume(settings.narrator_volume);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ui_language_defaults_roundtrips_and_validates() {
        let repo = SettingsRepository::open(Path::new(":memory:")).unwrap();
        let mut settings = repo.load().unwrap();
        assert_eq!(settings.ui_language, "auto");
        for preference in ["pl", "en", "auto"] {
            settings.ui_language = preference.into();
            repo.save(&settings).unwrap();
            assert_eq!(repo.load().unwrap(), settings);
        }
        let mut legacy = serde_json::to_value(&settings).unwrap();
        legacy.as_object_mut().unwrap().remove("uiLanguage");
        assert_eq!(
            serde_json::from_value::<AppSettings>(legacy)
                .unwrap()
                .ui_language,
            "auto"
        );
        settings.ui_language = "de".into();
        assert!(repo.save(&settings).is_err());
        assert_eq!(repo.load().unwrap().ui_language, "auto");
    }
    #[test]
    fn v13_adds_auto_language_without_changing_existing_settings() {
        let repo = SettingsRepository::open(Path::new(":memory:")).unwrap();
        repo.with_database(|db| {
            db.execute("ALTER TABLE settings DROP COLUMN ui_language", [])
                .unwrap();
            db.pragma_update(None, "user_version", 13).unwrap();
            db.execute("UPDATE settings SET interval_ms=3600000, language='pl'", [])
                .unwrap();
            migrate(db).unwrap();
            Ok(())
        })
        .unwrap();
        let settings = repo.load().unwrap();
        assert_eq!(settings.ui_language, "auto");
        assert_eq!(settings.language, "pl");
        assert_eq!(settings.interval_ms, 3600000);
    }
    fn schema(db: &Connection) -> Vec<(String, String, Option<String>)> {
        db.prepare("SELECT type,name,sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")
            .unwrap().query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap()
            .collect::<rusqlite::Result<_>>().unwrap()
    }
    fn history_counts(db: &Connection) -> Vec<i64> {
        [
            "news_articles",
            "news_usage",
            "content_broadcasts",
            "content_sources",
            "speech_assets",
            "article_materials",
            "article_history",
        ]
        .iter()
        .map(|table| {
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        })
        .collect()
    }
    #[test]
    fn current_schema_reopens_without_rewriting_settings_or_history() {
        let repo = SettingsRepository::open(Path::new(":memory:")).unwrap();
        let mut settings = repo.load().unwrap();
        settings.topics = vec!["Nauka".into()];
        settings.interval_ms = 3600000;
        repo.save(&settings).unwrap();
        repo.with_database(|db| {
            db.execute("INSERT INTO content_broadcasts VALUES (1,'now','Fixture','fixture','Zapisany scenariusz',1000,'[]','{}')",[]).unwrap();
            let before_schema=schema(db);let before_counts=history_counts(db);
            migrate(db).unwrap();migrate(db).unwrap();
            assert_eq!(schema(db),before_schema);assert_eq!(history_counts(db),before_counts);
            assert_eq!(db.pragma_query_value(None,"user_version",|r|r.get::<_,u32>(0)).unwrap(),14);
            assert_eq!(db.query_row("SELECT text FROM content_broadcasts WHERE id=1",[],|r|r.get::<_,String>(0)).unwrap(),"Zapisany scenariusz");
            Ok(())
        }).unwrap();
        assert_eq!(repo.load().unwrap(), settings);
    }
    #[test]
    fn v6_upgrades_without_removing_article_materials() {
        let mut db = Connection::open_in_memory().unwrap();
        for sql in [
            include_str!("../migrations/001_settings.sql"),
            include_str!("../migrations/002_news_preferences.sql"),
            include_str!("../migrations/003_news_history.sql"),
            include_str!("../migrations/004_content_generation.sql"),
            include_str!("../migrations/005_speech_assets.sql"),
            include_str!("../migrations/006_article_materials.sql"),
        ] {
            db.execute_batch(sql).unwrap();
        }
        db.pragma_update(None, "user_version", 6).unwrap();
        db.pragma_update(None, "foreign_keys", "ON").unwrap();
        db.execute("INSERT INTO news_articles(id,provider,canonical_url,metadata_json,first_seen_at) VALUES (1,'fixture','https://example.org','{}',0)",[]).unwrap();
        db.execute("INSERT INTO article_materials(article_id,title,status,fetched_at) VALUES (1,'Zachowany materiał','skipped',0)",[]).unwrap();
        migrate(&mut db).unwrap();
        assert_eq!(
            db.query_row("SELECT title FROM article_materials", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "Zachowany materiał"
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            14
        );
    }
    #[test]
    #[ignore = "explicit read-only verification of a local database, using a temporary SQLite snapshot"]
    fn verify_existing_database_on_copy() {
        let source_path =
            std::env::var_os("ARABRIEF_COMPAT_DATABASE").expect("set ARABRIEF_COMPAT_DATABASE");
        let source = Connection::open_with_flags(
            Path::new(&source_path),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let target = std::env::temp_dir().join(format!(
            "arabrief-compat-{}-{}.sqlite3",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        // SQLite makes a consistent snapshot; the source connection is read-only.
        source
            .execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])
            .unwrap();
        drop(source);
        {
            let before = Connection::open(&target).unwrap();
            let before_schema = schema(&before);
            let before_counts = history_counts(&before);
            // Capture every existing setting before migration, including v8 files
            // which do not yet have start_scheduler_on_launch.
            let mut statement = before.prepare("SELECT * FROM settings WHERE id=1").unwrap();
            let columns = statement.column_count();
            let before_values = statement
                .query_row([], |row| {
                    (0..columns)
                        .map(|index| row.get::<_, rusqlite::types::Value>(index))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap();
            drop(statement);
            drop(before);
            let repo = SettingsRepository::open(&target).unwrap();
            let settings = repo.load().unwrap();
            repo.save(&settings).unwrap();
            assert_eq!(repo.load().unwrap(), settings);
            repo.with_database(|db| {
                let after_values = db
                    .query_row("SELECT * FROM settings WHERE id=1", [], |row| {
                        (0..columns)
                            .map(|index| row.get::<_, rusqlite::types::Value>(index))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap();
                assert_eq!(after_values, before_values);
                let after_schema = schema(db);
                assert!(before_schema.iter().all(|entry| if entry.1 == "settings" {
                    after_schema
                        .iter()
                        .any(|after| after.0 == entry.0 && after.1 == entry.1)
                } else {
                    after_schema.contains(entry)
                }));
                assert_eq!(history_counts(db), before_counts);
                assert_eq!(
                    db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                        .unwrap(),
                    14
                );
                Ok(())
            })
            .unwrap();
        }
        std::fs::remove_file(target).unwrap();
        println!(
            "Verified: repository upgrades, settings roundtrip and history preserved on copy; original database read-only."
        );
    }
    #[test]
    fn defaults_roundtrip_and_invalid_save_preserves_data() {
        let repo = SettingsRepository::open(Path::new(":memory:")).unwrap();
        let mut settings = repo.load().unwrap();
        assert_eq!(settings.interval_ms, 1800000);
        assert_eq!(settings.duck_volume, 0.15);
        assert_eq!(settings.fade_out_ms, 3000);
        assert_eq!(settings.text_model, "gpt-5-mini");
        assert!(!settings.auto_start);
        assert!(!settings.start_scheduler_on_launch);
        assert_eq!(
            (
                settings.broadcast_min_duration_ms,
                settings.broadcast_max_duration_ms
            ),
            (120000, 180000)
        );
        settings.topics = vec!["Nauka".into(), "Świat".into()];
        settings.start_scheduler_on_launch = true;
        settings.preferred_audio_source = Some(PreferredAudioSource {
            id: "endpoint|instance".into(),
            name: "Muzyka".into(),
        });
        repo.save(&settings).unwrap();
        assert_eq!(repo.load().unwrap(), settings);
        let mut invalid = settings.clone();
        invalid.duck_volume = f32::NAN;
        assert!(repo.save(&invalid).is_err());
        assert_eq!(repo.load().unwrap(), settings);
        invalid = settings.clone();
        invalid.broadcast_min_duration_ms = 999999;
        assert!(invalid.validate().is_err());
        let mut json = serde_json::to_value(&settings).unwrap();
        json["apiKey"] = "not-a-real-key".into();
        assert!(serde_json::from_value::<AppSettings>(json).is_err());
    }
    #[test]
    fn upgrades_v1_without_reset_and_rejects_future_schema() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../migrations/001_settings.sql"))
            .unwrap();
        db.pragma_update(None, "user_version", 1).unwrap();
        db.execute("UPDATE settings SET interval_ms=3600000", [])
            .unwrap();
        migrate(&mut db).unwrap();
        migrate(&mut db).unwrap();
        assert_eq!(
            db.query_row("SELECT interval_ms FROM settings", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            3600000
        );
        db.pragma_update(None, "user_version", 99).unwrap();
        assert!(migrate(&mut db).is_err());
    }
    #[test]
    fn file_reopens_with_saved_preferences() {
        let path = std::env::temp_dir().join(format!(
            "arabrief-settings-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let repo = SettingsRepository::open(&path).unwrap();
            let mut s = repo.load().unwrap();
            s.open_ai_voice = "test-voice".into();
            s.text_model = "configured-model".into();
            repo.save(&s).unwrap();
        }
        {
            let repo = SettingsRepository::open(&path).unwrap();
            assert_eq!(repo.load().unwrap().open_ai_voice, "marin");
            assert_eq!(repo.load().unwrap().text_model, "configured-model");
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_migration_is_rolled_back() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../migrations/001_settings.sql"))
            .unwrap();
        db.pragma_update(None, "user_version", 1).unwrap();
        db.execute("CREATE TABLE settings_news_sources (url TEXT)", [])
            .unwrap();
        assert!(migrate(&mut db).is_err());
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert!(db.prepare("SELECT language FROM settings").is_err());
    }
}
