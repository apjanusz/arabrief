//! Native, bounded article reader. Never substitutes RSS snippets for a body.
use super::*;
use dom_query::Document;
use reqwest::Url;
use std::{
    net::{IpAddr, ToSocketAddrs},
    time::Instant,
};
#[cfg(test)]
mod tests;

const MAX_PAGE: u64 = 2 * 1024 * 1024;
const MAX_BODY: usize = 24_000;
#[derive(Clone, Debug)]
pub(crate) struct ArticleText {
    pub url: String,
    pub title: String,
    pub body: String,
}
struct Reply {
    status: u16,
    location: Option<String>,
    body: String,
}
trait Http {
    fn send(&self, url: &Url, form: Option<&str>) -> Result<Reply, String>;
}
struct NativeHttp {
    deadline: Instant,
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 240
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])))
        }
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or_else(
            || {
                // Only global unicast, excluding documentation and local scopes.
                ip.segments()[0] & 0xe000 == 0x2000
                    && !(ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
            },
            |ip| public_ip(IpAddr::V4(ip)),
        ),
    }
}
fn valid_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.port_or_known_default(), Some(80 | 443))
        && url.host_str().is_some_and(|host| {
            host.contains('.') && host != "localhost" && !host.ends_with(".local")
        })
        && url.host_str().is_some_and(|host| {
            host.trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .map_or(true, public_ip)
        })
}
fn host(url: &Url) -> String {
    url.host_str()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_lowercase()
}
fn allowed(source: &NewsItem, url: &Url) -> bool {
    if !valid_url(url) {
        return false;
    }
    let original = Url::parse(&source.url).ok();
    let google = original
        .as_ref()
        .is_some_and(|url| host(url) == "news.google.com");
    if google && host(url) == "news.google.com" {
        return true;
    }
    let publisher = if google {
        source
            .source_url
            .as_deref()
            .and_then(|raw| Url::parse(raw).ok())
    } else {
        original
    };
    publisher.is_some_and(|publisher| {
        let expected = host(&publisher);
        let actual = host(url);
        !expected.is_empty() && (actual == expected || actual.ends_with(&format!(".{expected}")))
    })
}
impl Http for NativeHttp {
    fn send(&self, url: &Url, form: Option<&str>) -> Result<Reply, String> {
        if !valid_url(url) {
            return Err("Niedozwolony adres artykułu".into());
        }
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(12));
        if remaining.is_zero() {
            return Err("Przekroczono czas pobierania artykułu".into());
        }
        let host = url.host_str().ok_or("Brak domeny artykułu")?;
        let addresses: Vec<_> = (host, url.port_or_known_default().unwrap_or(443))
            .to_socket_addrs()
            .map_err(|_| "Nie znaleziono serwera artykułu")?
            .collect();
        if addresses.is_empty() || addresses.iter().any(|addr| !public_ip(addr.ip())) {
            return Err("Zablokowano lokalny lub prywatny adres artykułu".into());
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(remaining)
            .connect_timeout(remaining.min(Duration::from_secs(5)))
            .resolve_to_addrs(host, &addresses)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AraBrief/0.1")
            .build()
            .map_err(|_| "Błąd czytnika artykułów")?;
        let request = if let Some(form) = form {
            client
                .post(url.clone())
                .header(
                    CONTENT_TYPE,
                    "application/x-www-form-urlencoded;charset=UTF-8",
                )
                .body(form.to_owned())
        } else {
            client.get(url.clone())
        };
        // Stateless redirect hint used only for Google News; never read or
        // forward the user's browser cookies to publisher sites.
        let request = if url.host_str() == Some("news.google.com") {
            request.header(reqwest::header::COOKIE, "SOCS=CAI")
        } else {
            request
        };
        let response = request
            .header("Accept-Language", "pl-PL,pl;q=0.9,en;q=0.5")
            .send()
            .map_err(|_| "Niedostępna strona artykułu lub przekroczony czas HTTP")?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        if status >= 300 {
            return Ok(Reply {
                status,
                location,
                body: String::new(),
            });
        }
        if response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                !value.starts_with("text/")
                    && !value.starts_with("application/json")
                    && !value.starts_with("application/xhtml+xml")
            })
        {
            return Err("Artykuł nie jest stroną tekstową".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_PAGE + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Nie odczytano strony artykułu")?;
        if bytes.len() as u64 > MAX_PAGE {
            return Err("Strona artykułu przekracza 2 MiB".into());
        }
        let body = String::from_utf8(bytes)
            .map_err(|_| "Nieobsługiwane kodowanie strony artykułu (wymagane UTF-8)")?;
        Ok(Reply {
            status,
            location,
            body,
        })
    }
}
fn get(source: &NewsItem, mut url: Url, http: &impl Http) -> Result<(Url, String), String> {
    for _ in 0..5 {
        if !allowed(source, &url) {
            return Err(format!(
                "Przekierowanie poza domenę wydawcy ({})",
                host(&url)
            ));
        }
        let reply = http.send(&url, None)?;
        if (300..400).contains(&reply.status) {
            url = url
                .join(
                    reply
                        .location
                        .as_deref()
                        .ok_or("Brak celu przekierowania")?,
                )
                .map_err(|_| "Błędne przekierowanie artykułu")?;
            continue;
        }
        if reply.status != 200 {
            return Err(format!("Strona artykułu: HTTP {}", reply.status));
        }
        return Ok((url, reply.body));
    }
    Err("Zbyt wiele przekierowań artykułu".into())
}
// Google has no documented RSS URL-resolution API. This mirrors its redirect
// page's RPC protocol; isolate it so a Google change only fails this article.
fn google_form(id: &str, signature: &str, timestamp: i64) -> String {
    let payload = json!([
        "garturlreq",
        [
            [
                "X",
                "X",
                ["X", "X"],
                null,
                null,
                1,
                1,
                "US:en",
                null,
                1,
                null,
                null,
                null,
                null,
                null,
                0,
                1
            ],
            "X",
            "X",
            1,
            [1, 1, 1],
            1,
            1,
            null,
            0,
            0,
            null,
            0
        ],
        id,
        timestamp,
        signature
    ]);
    let outer = json!([[["Fbv4je", payload.to_string(), null, "generic"]]]);
    url_form(&outer.to_string())
}
fn url_form(value: &str) -> String {
    reqwest::Url::parse_with_params("https://news.google.com/", [("f.req", value)])
        .expect("constant URL")
        .query()
        .unwrap()
        .to_owned()
}
fn google_result(body: &str) -> Result<Url, String> {
    for line in body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("[["))
    {
        if let Ok(Value::Array(rows)) = serde_json::from_str::<Value>(line) {
            for row in rows {
                if row[1] == "Fbv4je"
                    && let Some(raw) = row[2].as_str()
                    && let Ok(value) = serde_json::from_str::<Value>(raw)
                    && value[0] == "garturlres"
                    && let Some(url) = value[1].as_str()
                {
                    return Url::parse(url)
                        .map_err(|_| "Błędny adres wydawcy z Google News".into());
                }
            }
        }
    }
    Err("Google News nie zwrócił adresu artykułu wydawcy".into())
}
fn read(source: &NewsItem, http: &impl Http) -> Result<ArticleText, String> {
    let original = Url::parse(&source.url).map_err(|_| "Błędny adres artykułu")?;
    let (mut url, mut html) = get(source, original.clone(), http)?;
    if host(&url) == "news.google.com" {
        let doc = Document::from(html.as_str());
        let element = doc.select("[data-n-a-sg][data-n-a-ts]");
        let signature = element
            .attr("data-n-a-sg")
            .ok_or("Google News nie udostępnił przekierowania do wydawcy")?;
        let timestamp = element
            .attr("data-n-a-ts")
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or("Brak znacznika przekierowania Google News")?;
        let id = original
            .path_segments()
            .and_then(|mut parts| parts.next_back())
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 4096
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            })
            .ok_or("Błędny identyfikator Google News")?;
        let rpc =
            Url::parse("https://news.google.com/_/DotsSplashUi/data/batchexecute?rpcids=Fbv4je")
                .unwrap();
        let reply = http.send(&rpc, Some(&google_form(id, &signature, timestamp)))?;
        if reply.status != 200 {
            return Err(format!("Przekierowanie Google News: HTTP {}", reply.status));
        }
        (url, html) = get(source, google_result(&reply.body)?, http)?;
    }
    if host(&url) == "news.google.com" || url.path().trim_matches('/').is_empty() {
        return Err("Otrzymano stronę pośrednią zamiast artykułu".into());
    }
    extract(&html, url.as_str(), &source.title)
}
pub(super) fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn sufficient(body: &str) -> bool {
    body.chars().count() >= 400 && body.split_whitespace().count() >= 60
}
fn title_matches(expected: &str, actual: &str) -> bool {
    let expected = expected
        .rsplit_once(" - ")
        .map_or(expected, |(title, _)| title);
    let tokens = |s: &str| {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| s.chars().count() >= 3)
            .map(str::to_owned)
            .collect::<HashSet<_>>()
    };
    let expected = tokens(expected);
    let actual = tokens(actual);
    !expected.is_empty() && expected.intersection(&actual).count() * 100 >= expected.len() * 60
}
fn structured(value: &Value, depth: usize) -> Option<(String, String)> {
    if depth > 12 {
        return None;
    }
    if let Some(object) = value.as_object() {
        let article = object.get("@type").is_some_and(|kind| {
            kind.as_str()
                .is_some_and(|s| s.ends_with("Article") || s == "BlogPosting")
                || kind.as_array().is_some_and(|v| {
                    v.iter().any(|s| {
                        s.as_str()
                            .is_some_and(|s| s.ends_with("Article") || s == "BlogPosting")
                    })
                })
        });
        if article
            && object.get("isAccessibleForFree") != Some(&json!(false))
            && object.get("isAccessibleForFree") != Some(&json!("false"))
            && let (Some(title), Some(body)) = (
                object.get("headline").and_then(Value::as_str),
                object.get("articleBody").and_then(Value::as_str),
            )
        {
            return Some((title.to_owned(), body.to_owned()));
        }
        for child in object.values() {
            if let Some(result) = structured(child, depth + 1) {
                return Some(result);
            }
        }
    } else if let Some(items) = value.as_array() {
        for child in items {
            if let Some(result) = structured(child, depth + 1) {
                return Some(result);
            }
        }
    }
    None
}
fn extract(html: &str, url: &str, expected_title: &str) -> Result<ArticleText, String> {
    let doc = Document::from(html);
    // Fail closed on explicit paywall metadata, never scrape around a paywall.
    let metadata: Vec<Value> = doc
        .select("script[type='application/ld+json']")
        .iter()
        .filter_map(|script| serde_json::from_str(&script.text()).ok())
        .collect();
    for value in &metadata {
        if value.to_string().contains("\"isAccessibleForFree\":false")
            || value
                .to_string()
                .contains("\"isAccessibleForFree\":\"false\"")
        {
            return Err("Artykuł jest oznaczony jako płatny".into());
        }
    }
    for value in &metadata {
        if let Some((title, body)) = structured(value, 0) {
            let body = normalized(&Document::fragment(body.as_str()).text());
            if title_matches(expected_title, &title) && sufficient(&body) {
                return Ok(ArticleText {
                    url: url.into(),
                    title,
                    body: body.chars().take(MAX_BODY).collect(),
                });
            }
        }
    }
    doc.select("script,style,nav,header,footer,aside,form,dialog,noscript,[hidden],[aria-hidden='true'],[class*='comment'],[class*='paywall']").remove();
    let title = normalized(&doc.select("h1").first().text());
    if !title_matches(expected_title, &title) {
        return Err("Tytuł strony nie odpowiada wybranemu artykułowi".into());
    }
    for selector in [
        "[itemprop='articleBody']",
        "article",
        "main",
        "[role='main']",
    ] {
        for container in doc.select(selector).iter() {
            let mut seen = HashSet::new();
            let paragraphs: Vec<_> = container
                .select("p")
                .iter()
                .map(|p| normalized(&p.text()))
                .filter(|p| p.chars().count() >= 40 && seen.insert(p.clone()))
                .collect();
            let body = paragraphs.join("\n\n");
            if paragraphs.len() >= 2 && sufficient(&body) {
                return Ok(ArticleText {
                    url: url.into(),
                    title,
                    body: body.chars().take(MAX_BODY).collect(),
                });
            }
        }
    }
    Err("Nie uzyskano pełnego tekstu artykułu (tylko zajawka, blokada lub strona wymaga JavaScript)".into())
}
pub(crate) fn load(
    repository: &SettingsRepository,
    source: &NewsItem,
) -> Result<ArticleText, String> {
    load_with(
        repository,
        source,
        &NativeHttp {
            deadline: Instant::now() + Duration::from_secs(40),
        },
    )
}
fn load_with(
    repository: &SettingsRepository,
    source: &NewsItem,
    http: &impl Http,
) -> Result<ArticleText, String> {
    let cached = repository.with_database(|db| db.query_row("SELECT publisher_url,title,body_text FROM article_materials WHERE article_id=?1 AND status='usable' AND reason='native_article_v1'", [&source.id], |r| Ok(ArticleText { url: r.get(0)?, title: r.get(1)?, body: r.get(2)? })).optional().map_err(|e|e.to_string()))?;
    if let Some(text) = cached
        && sufficient(&text.body)
        && text.body.chars().count() <= MAX_BODY
        && title_matches(&source.title, &text.title)
        && Url::parse(&text.url).is_ok_and(|url| allowed(source, &url))
    {
        return Ok(text);
    }
    let text = read(source, http)?;
    repository.with_database(|db| db.execute("INSERT INTO article_materials(article_id,publisher_url,title,publisher,published_at,body_text,status,reason,fetched_at) VALUES (?1,?2,?3,?4,?5,?6,'usable','native_article_v1',unixepoch()) ON CONFLICT(article_id) DO UPDATE SET publisher_url=excluded.publisher_url,title=excluded.title,body_text=excluded.body_text,status=excluded.status,reason=excluded.reason,fetched_at=excluded.fetched_at", params![source.id,text.url,text.title,source.source,source.published_at,text.body]).map(|_|()).map_err(|e|e.to_string()))?;
    Ok(text)
}
