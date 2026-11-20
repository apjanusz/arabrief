//! Cache only complete, transcript-verified PCM units. Never publish a gapped recording.
use super::format;
use crate::settings::SettingsRepository;
use rusqlite::{OptionalExtension, params};
pub(super) type Audio = (Vec<u8>, String);
pub(super) fn load(
    repository: &SettingsRepository,
    text: &str,
    voice: &str,
) -> Result<Option<Audio>, String> {
    let settings = repository.load()?;
    load_for_settings(repository, text, voice, &settings)
}
pub(super) fn load_for_settings(
    repository: &SettingsRepository,
    text: &str,
    voice: &str,
    settings: &crate::settings::AppSettings,
) -> Result<Option<Audio>, String> {
    repository.with_database(|db|{
        let found=db.query_row("SELECT pcm,transcript,pcm_hash FROM speech_segments WHERE cache_key=?1 AND created_at>?2",params![format::contextual_cache_key(text,voice,settings),chrono::Utc::now().timestamp()-86400],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional().map_err(|e|e.to_string())?;
        if let Some((pcm,transcript,hash))=found {
            if !pcm.is_empty()&&pcm.len()<=8*1024*1024&&pcm.len().is_multiple_of(2)&&format::hash(&pcm)==hash&&format::verify(text,&transcript).is_ok(){return Ok(Some((pcm,transcript)));}
            crate::performance::diagnostic!("[recovery] stage=speech action=skipped reason=corrupt_segment_cache");
        }Ok(None)
    })
}
#[cfg(test)]
pub(super) fn save(
    repository: &SettingsRepository,
    text: &str,
    voice: &str,
    audio: &Audio,
) -> Result<(), String> {
    let settings = repository.load()?;
    save_for_settings(repository, text, voice, audio, &settings)
}
pub(super) fn save_for_settings(
    repository: &SettingsRepository,
    text: &str,
    voice: &str,
    audio: &Audio,
    settings: &crate::settings::AppSettings,
) -> Result<(), String> {
    format::verify(text, &audio.1)?;
    if audio.0.is_empty() || audio.0.len() > 8 * 1024 * 1024 || !audio.0.len().is_multiple_of(2) {
        return Err("Nieprawidłowy PCM segmentu".into());
    }
    repository.with_database(|db|{
        let tx=db.transaction().map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM speech_segments WHERE created_at<?1",[chrono::Utc::now().timestamp()-86400]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO speech_segments VALUES (?1,?2,?3,?4,?5) ON CONFLICT(cache_key) DO UPDATE SET pcm=excluded.pcm,transcript=excluded.transcript,pcm_hash=excluded.pcm_hash,created_at=excluded.created_at",params![format::contextual_cache_key(text,voice,settings),audio.0,audio.1,format::hash(&audio.0),chrono::Utc::now().timestamp()]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_text_is_not_reused_between_languages_or_regions() {
        let repo = SettingsRepository::open(std::path::Path::new(":memory:")).unwrap();
        let mut settings = repo.load().unwrap();
        settings.ui_language = "pl".into();
        settings.news_region = "US".into();
        repo.save(&settings).unwrap();
        save(&repo, "Test", "marin", &(vec![0; 480], "Test".into())).unwrap();
        assert!(load(&repo, "Test", "marin").unwrap().is_some());
        settings.ui_language = "en".into();
        repo.save(&settings).unwrap();
        assert!(load(&repo, "Test", "marin").unwrap().is_none());
        settings.ui_language = "pl".into();
        settings.news_region = "GB".into();
        repo.save(&settings).unwrap();
        assert!(load(&repo, "Test", "marin").unwrap().is_none());
    }
    #[test]
    fn preserves_completed_parts_after_later_failure_and_reuses_only_matching_voice_and_text() {
        let repository = SettingsRepository::open(std::path::Path::new(":memory:")).unwrap();
        let first = "Pierwsza wiadomość pozostaje poprawna.";
        let second = "Druga wiadomość wymaga ponowienia.";
        save(&repository, first, "alloy", &(vec![0; 48000], first.into())).unwrap();
        assert!(
            save(
                &repository,
                second,
                "alloy",
                &(vec![0; 48000], "Nieznany fakt".into())
            )
            .is_err()
        );
        assert!(load(&repository, first, "alloy").unwrap().is_some());
        assert!(load(&repository, second, "alloy").unwrap().is_none());
        assert!(load(&repository, first, "coral").unwrap().is_none());
        save(
            &repository,
            second,
            "alloy",
            &(vec![0; 48000], second.into()),
        )
        .unwrap();
        assert!(load(&repository, second, "alloy").unwrap().is_some());
        repository
            .with_database(|db| {
                db.execute("UPDATE speech_segments SET pcm=x'00'", [])
                    .unwrap();
                Ok(())
            })
            .unwrap();
        assert!(load(&repository, first, "alloy").unwrap().is_none());
    }
    #[tokio::test]
    async fn completed_segment_cache_assembles_without_connection_or_api_key() {
        let repository = std::sync::Arc::new(
            SettingsRepository::open(std::path::Path::new(":memory:")).unwrap(),
        );
        let parts = vec![
            "Pierwsza poprawna wiadomość.".to_owned(),
            "Druga poprawna wiadomość.".to_owned(),
        ];
        for part in &parts {
            save(&repository, part, "alloy", &(vec![0; 48000], part.clone())).unwrap();
        }
        let (_sender, mut cancel) = tokio::sync::watch::channel(false);
        let (pcm, transcript) =
            super::super::realtime::synthesize(&parts, "alloy", &mut cancel, repository)
                .await
                .unwrap();
        assert_eq!(pcm.len(), 96000);
        assert_eq!(transcript, parts.join("\n\n"));
    }
}
