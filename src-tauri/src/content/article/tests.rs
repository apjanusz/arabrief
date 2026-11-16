use super::*;
use std::{cell::RefCell, collections::VecDeque};
fn source(url: &str) -> NewsItem {
    NewsItem {
        id: "1".into(),
        provider: "google_news".into(),
        title: "Naukowcy opisali wyniki nowych badań".into(),
        url: url.into(),
        source: Some("Wydawca".into()),
        source_url: Some("https://publisher.example".into()),
        published_at: None,
        raw_publication_date: None,
        description_html: Some("Tylko zajawka RSS".into()),
        guid: None,
        feed_url: "https://news.google.com/rss/search".into(),
        fetched_at: Utc::now().to_rfc3339(),
        topics: vec!["Nauka".into()],
    }
}
fn body() -> String {
    (1..=8).map(|i| format!("Zespół numer {i} przeanalizował wyniki pomiarów temperatury w laboratorium. Badacze opisali metody oraz ograniczenia eksperymentu, opublikowali szczegółowe dane i zapowiedzieli dalsze obserwacje." )).collect::<Vec<_>>().join("\n\n")
}
fn html() -> String {
    format!(
        "<html><h1>Naukowcy opisali wyniki nowych badań</h1><nav><p>NAWIGACJA NIE JEST TREŚCIĄ</p></nav><article>{}</article><footer>STOPKA</footer></html>",
        body()
            .split("\n\n")
            .map(|p| format!("<p>{p}</p>"))
            .collect::<String>()
    )
}
struct FakeHttp {
    replies: RefCell<VecDeque<Reply>>,
    requests: RefCell<Vec<String>>,
}
impl FakeHttp {
    fn new(replies: Vec<Reply>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            requests: RefCell::new(vec![]),
        }
    }
}
impl Http for FakeHttp {
    fn send(&self, url: &Url, _: Option<&str>) -> Result<Reply, String> {
        self.requests.borrow_mut().push(url.to_string());
        self.replies
            .borrow_mut()
            .pop_front()
            .ok_or("Nieoczekiwane powtórne pobranie".into())
    }
}
fn ok(body: String) -> Reply {
    Reply {
        status: 200,
        location: None,
        body,
    }
}
#[test]
fn html_and_structured_bodies_are_real_text_not_descriptions_or_navigation() {
    let source = source("https://publisher.example/story");
    let extracted = extract(&html(), &source.url, &source.title).unwrap();
    assert!(extracted.body.contains("laboratorium"));
    assert!(!extracted.body.contains("NAWIGACJA"));
    assert!(!extracted.body.contains("STOPKA"));
    assert!(!extracted.body.contains("zajawka RSS"));
    let data =
        json!({"@graph":[{"@type":"NewsArticle","headline":source.title,"articleBody":body()}]});
    assert!(
        extract(
            &format!("<script type='application/ld+json'>{data}</script>"),
            &source.url,
            &source.title
        )
        .is_ok()
    );
    assert!(extract("<h1>Naukowcy opisali wyniki nowych badań</h1><article><p>Tylko krótka zajawka.</p></article>",&source.url,&source.title).is_err());
    assert!(extract(&html(), &source.url, "Plebiscyt sportowy w Malezji").is_err());
    let paywall = format!(
        "{}<script type='application/ld+json'>{{\"isAccessibleForFree\":false}}</script>",
        html()
    );
    assert!(
        extract(&paywall, &source.url, &source.title)
            .unwrap_err()
            .contains("płatny")
    );
}
#[test]
fn google_resolver_verifies_publisher_and_cache_survives_restart() {
    let path = std::env::temp_dir().join(format!(
        "arabrief-native-{}-{}.sqlite",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let source = source("https://news.google.com/rss/articles/ABC_123");
    let frame = json!([[
        "wrb.fr",
        "Fbv4je",
        json!(["garturlres", "https://publisher.example/story", 1]).to_string()
    ]]);
    let http = FakeHttp::new(vec![
        ok("<div data-n-a-sg='signature' data-n-a-ts='123'></div>".into()),
        ok(format!(")]}}'\n\n123\n{frame}\n")),
        ok(html()),
    ]);
    {
        let repo = SettingsRepository::open(&path).unwrap();
        repo.with_database(|db| { db.execute("INSERT INTO news_articles(id,provider,canonical_url,metadata_json,first_seen_at) VALUES (1,'google_news',?1,?2,unixepoch())", params![source.url,serde_json::to_string(&source).unwrap()]).map(|_|()).map_err(|e|e.to_string()) }).unwrap();
        let material = load_with(&repo, &source, &http).unwrap();
        assert_eq!(material.url, "https://publisher.example/story");
        assert_eq!(http.requests.borrow().len(), 3);
    }
    {
        let repo = SettingsRepository::open(&path).unwrap();
        assert!(load_with(&repo, &source, &FakeHttp::new(vec![])).is_ok());
    }
    std::fs::remove_file(path).unwrap();
    let form = google_form("ABC_123", "signature", 123);
    assert!(form.starts_with("f.req="));
}
#[test]
fn unavailable_pages_and_unsafe_redirects_fail_before_generation() {
    let source = source("https://publisher.example/story");
    for status in [403, 404, 429, 500] {
        let http = FakeHttp::new(vec![Reply {
            status,
            location: None,
            body: String::new(),
        }]);
        assert!(read(&source, &http).is_err());
        assert_eq!(http.requests.borrow().len(), 1);
    }
    let http = FakeHttp::new(vec![Reply {
        status: 302,
        location: Some("http://127.0.0.1/private".into()),
        body: String::new(),
    }]);
    assert!(read(&source, &http).is_err());
    assert_eq!(http.requests.borrow().len(), 1);
    assert!(!allowed(
        &source,
        &Url::parse("https://evil.example/story").unwrap()
    ));
    for raw in [
        "127.0.0.1",
        "10.1.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "::1",
        "::ffff:127.0.0.1",
        "fc00::1",
    ] {
        assert!(!public_ip(raw.parse().unwrap()), "{raw}");
    }
    assert!(public_ip("8.8.8.8".parse().unwrap()));
    assert!(google_result("bad response").is_err());
}

#[test]
#[ignore = "explicit public HTTP smoke test; no OpenAI requests or credentials"]
fn live_google_article_reader_without_openai() {
    let feed = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap()
        .get("https://news.google.com/rss/search?q=wspinaczka+site:wspinanie.pl+when:7d&hl=pl&gl=PL&ceid=PL:pl")
        .send()
        .expect("Google RSS HTTP")
        .error_for_status()
        .unwrap()
        .bytes()
        .unwrap();
    let feed = rss::Channel::read_from(feed.as_ref()).unwrap();
    let mut successes = 0;
    for item in feed.items().iter().take(3) {
        let mut source = source(item.link().unwrap());
        source.title = item.title().unwrap().into();
        source.source_url = item.source().map(|s| s.url().to_owned());
        let result = read(
            &source,
            &NativeHttp {
                deadline: Instant::now() + Duration::from_secs(40),
            },
        );
        match result {
            Ok(text) => {
                successes += 1;
                println!("Native reader: {} characters", text.body.chars().count());
            }
            Err(error) => println!(
                "Native reader ({}): {error}",
                source.source_url.as_deref().unwrap_or("unknown publisher")
            ),
        }
    }
    assert!(
        successes > 0,
        "No publisher page could be read; inspect HTTP/Google errors above"
    );
}
