use super::*;
use serde_json::json;
const TEXTS: [&str; 3] = [
    "Pierwszy rozdział opisuje szczegółowo wyniki pomiarów zespołu badaczy.",
    "Drugi rozdział przedstawia warunki doświadczenia i ograniczenia metody.",
    "Trzeci rozdział wyjaśnia konkretne plany kolejnych badań naukowych.",
];
fn setup() -> (Arc<SpeechService>, PathBuf) {
    let repository = Arc::new(SettingsRepository::open(Path::new(":memory:")).unwrap());
    let key = crate::recovery::settings_key(&repository.load().unwrap());
    repository.with_database(|db|{
        let draft=json!({"title":"Audycja","segments":TEXTS.iter().enumerate().map(|(i,text)|json!({"topic":"Nauka","text":text,"evidence":[{"source_id":(i+1).to_string(),"quote":"Szczegółowe informacje wydawcy"}]})).collect::<Vec<_>>(),"limitations":[],"insufficient_material":false});
        db.execute("INSERT INTO content_broadcasts VALUES (1,?1,'Audycja','fixture',?2,1000,'[]',?3)",params![chrono::Utc::now().to_rfc3339(),TEXTS.join("\n\n"),draft.to_string()]).unwrap();db.execute("INSERT INTO pipeline_preparations VALUES(1,?1)",[key]).unwrap();
        for (index,text) in TEXTS.iter().enumerate(){let id=(index+1) as u32;let source=json!({"id":id.to_string(),"provider":"fixture","title":format!("Wiadomość {id}"),"url":format!("https://example.org/{id}"),"source":"Fixture","sourceUrl":"https://example.org","publishedAt":null,"rawPublicationDate":null,"descriptionHtml":null,"guid":id.to_string(),"feedUrl":"https://example.org/rss","fetchedAt":chrono::Utc::now().to_rfc3339(),"topics":["Nauka"]});
            db.execute("INSERT INTO news_articles(id,provider,canonical_url,metadata_json,first_seen_at) VALUES(?1,'fixture',?2,?3,0)",params![id,format!("https://example.org/{id}"),source.to_string()]).unwrap();db.execute("INSERT INTO article_queue(article_id) VALUES(?1)",[id]).unwrap();
            db.execute("INSERT INTO content_sources VALUES(1,?1,?2)",params![id,source.to_string()]).unwrap();db.execute("INSERT INTO broadcast_chapters(broadcast_id,article_id,position,text) VALUES(1,?1,?2,?3)",params![id,index as u32,text]).unwrap();
        }Ok(())
    }).unwrap();
    let root = std::env::temp_dir().join(format!(
        "arabrief-chapters-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    (Arc::new(SpeechService::new(repository)), root)
}
struct Provider {
    calls: Mutex<Vec<String>>,
    fail_all: bool,
    mismatch: bool,
    cancel: Option<watch::Sender<bool>>,
}
impl Provider {
    fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
            fail_all: false,
            mismatch: false,
            cancel: None,
        }
    }
}
impl SpeechProvider for Provider {
    async fn synthesize<'a>(
        &'a self,
        parts: &'a [String],
        _: &'a str,
        _: &'a mut watch::Receiver<bool>,
    ) -> Result<(Vec<u8>, String), String> {
        let text = parts.join("\n\n");
        self.calls.lock().unwrap().push(text.clone());
        if let Some(cancel) = &self.cancel {
            cancel.send(true).unwrap();
            return Err("Anulowano".into());
        }
        if self.fail_all || text.starts_with("Drugi") && !self.mismatch {
            return Err("Niedostępna mowa tego rozdziału".into());
        }
        Ok((
            vec![0; 48000],
            if self.mismatch && text.starts_with("Drugi") {
                "Nieprawidłowy zapis wypowiedzi".into()
            } else {
                text
            },
        ))
    }
}
#[tokio::test]
async fn failed_middle_tts_keeps_text_and_publishes_ordered_siblings_with_boundaries() {
    let (service, root) = setup();
    let provider = Provider::new();
    let (_sender, mut cancel) = watch::channel(false);
    let asset = service
        .clone()
        .generate(
            root.clone(),
            "1".into(),
            Some("alloy".into()),
            &mut cancel,
            &provider,
        )
        .await
        .unwrap();
    assert_eq!(
        asset
            .chapters
            .iter()
            .map(|span| span.article_id.as_str())
            .collect::<Vec<_>>(),
        ["1", "3"]
    );
    assert_eq!(asset.chapters[0].start_ms, 0);
    assert_eq!(asset.chapters[0].end_ms, 1000);
    assert_eq!(asset.chapters[1].start_ms, 1000);
    assert_eq!(asset.duration_ms, 2000);
    assert_eq!(asset.transcript, [TEXTS[0], TEXTS[2]].join("\n\n"));
    assert_eq!(*provider.calls.lock().unwrap(), TEXTS);
    service
        .repository
        .with_database(|db| {
            assert_eq!(
                db.query_row(
                    "SELECT text FROM broadcast_chapters WHERE broadcast_id=1 AND article_id=2",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                TEXTS[1]
            );
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM content_sources WHERE broadcast_id=1",
                    [],
                    |r| r.get::<_, u32>(0)
                )
                .unwrap(),
                2
            );
            assert_eq!(
                db.query_row("SELECT count(*) FROM news_usage", [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                0
            );
            Ok(())
        })
        .unwrap();
    crate::content::load_saved(&service.repository, 1, &service.repository.load().unwrap())
        .unwrap();
    let reused = service
        .clone()
        .generate(
            root.clone(),
            "1".into(),
            Some("alloy".into()),
            &mut cancel,
            &provider,
        )
        .await
        .unwrap();
    assert!(reused.cached);
    assert_eq!(provider.calls.lock().unwrap().len(), 3);
    assert_eq!(reused.chapters.len(), 2);
    super::super::tests::clean(&root);
}
#[tokio::test]
async fn bad_transcript_drops_only_its_chapter() {
    let (service, root) = setup();
    let mut provider = Provider::new();
    provider.mismatch = true;
    let (_sender, mut cancel) = watch::channel(false);
    let asset = service
        .generate(
            root.clone(),
            "1".into(),
            Some("alloy".into()),
            &mut cancel,
            &provider,
        )
        .await
        .unwrap();
    assert_eq!(asset.chapters.len(), 2);
    super::super::tests::clean(&root);
}
#[tokio::test]
async fn all_tts_failures_release_reservations_without_erasing_texts() {
    let (service, root) = setup();
    let mut provider = Provider::new();
    provider.fail_all = true;
    let (_sender, mut cancel) = watch::channel(false);
    assert!(
        service
            .clone()
            .generate(
                root,
                "1".into(),
                Some("alloy".into()),
                &mut cancel,
                &provider
            )
            .await
            .is_err()
    );
    service
        .repository
        .with_database(|db| {
            assert_eq!(
                db.query_row("SELECT count(*) FROM pipeline_preparations", [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                db.query_row("SELECT count(*) FROM broadcast_chapters", [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                3
            );
            assert_eq!(
                db.query_row("SELECT text FROM content_broadcasts WHERE id=1", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
                TEXTS.join("\n\n")
            );
            Ok(())
        })
        .unwrap();
}
#[tokio::test]
async fn cancellation_does_not_consume_or_discard_chapters() {
    let (service, root) = setup();
    let (sender, mut cancel) = watch::channel(false);
    let mut provider = Provider::new();
    provider.cancel = Some(sender);
    assert!(
        service
            .clone()
            .generate(
                root,
                "1".into(),
                Some("alloy".into()),
                &mut cancel,
                &provider
            )
            .await
            .is_err()
    );
    service
        .repository
        .with_database(|db| {
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM broadcast_chapters WHERE state='text_ready'",
                    [],
                    |r| r.get::<_, u32>(0)
                )
                .unwrap(),
                3
            );
            assert_eq!(
                db.query_row("SELECT count(*) FROM news_usage", [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                0
            );
            Ok(())
        })
        .unwrap();
}
