//! Cheap topical admission, independent of duplicate detection and AI calls.
use super::NewsItem;

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        })
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}
fn has(words: &[String], stems: &[&str]) -> bool {
    words
        .iter()
        .any(|word| stems.iter().any(|stem| word.starts_with(stem)))
}
fn climbing_topic(words: &[String]) -> bool {
    has(words, &["wspinac"])
        && words
            .iter()
            .all(|word| word.starts_with("wspinac") || word.starts_with("sport"))
}
fn sports_nutrition_topic(words: &[String]) -> bool {
    words.len() == 2 && has(words, &["diet", "zywieni", "odzywian"]) && has(words, &["sport"])
}
fn plain_description(html: &str) -> String {
    let mut tag = false;
    html.chars()
        .map(|c| match c {
            '<' => {
                tag = true;
                ' '
            }
            '>' => {
                tag = false;
                ' '
            }
            _ if tag => ' ',
            _ => c,
        })
        .collect()
}
pub(super) fn matches(topic: &str, item: &NewsItem) -> bool {
    // Publisher names and URL/HTML attributes must not supply topic keywords.
    let title = item
        .title
        .rsplit_once(" - ")
        .map_or(item.title.as_str(), |(title, _)| title);
    let mut description = plain_description(item.description_html.as_deref().unwrap_or(""));
    if let Some(source) = item.source.as_ref().filter(|source| !source.is_empty()) {
        description = description.replace(source, " ");
    }
    let text = words(&format!("{title} {description}"));
    let topic = words(topic);
    if climbing_topic(&topic) {
        return has(&text, &["wspinac", "boulder", "skalk"]);
    }
    if sports_nutrition_topic(&topic) {
        return has(
            &text,
            &[
                "diet",
                "zywieni",
                "odzyw",
                "bialk",
                "weglowodan",
                "suplement",
                "kreatyn",
                "posilk",
                "kalori",
                "nawodn",
            ],
        ) && has(
            &text,
            &[
                "sport",
                "trening",
                "bieg",
                "zawodnik",
                "atlet",
                "wysilk",
                "triathlon",
                "kulturyst",
            ],
        );
    }
    let significant: Vec<_> = topic
        .iter()
        .filter(|word| {
            ![
                "i", "a", "o", "w", "we", "z", "ze", "na", "do", "dla", "oraz", "and", "the", "of",
            ]
            .contains(&word.as_str())
        })
        .collect();
    !significant.is_empty()
        && significant.iter().all(|word| {
            let length = word.chars().count();
            let stem: String = word
                .chars()
                .take(if length >= 5 { length - 1 } else { length })
                .collect();
            text.iter().any(|candidate| {
                if length < 4 {
                    candidate == *word
                } else {
                    candidate.starts_with(&stem)
                }
            })
        })
}
pub(super) fn query(topic: &str) -> String {
    let tokens = words(topic);
    if climbing_topic(&tokens) {
        return "(\"wspinaczka\" OR \"wspinaczki\" OR \"wspinaczkowe\" OR \"bouldering\")".into();
    }
    if sports_nutrition_topic(&tokens) {
        return "(\"dieta\" OR \"żywienie\" OR \"odżywianie\" OR \"suplementacja\") (\"sportowców\" OR \"sportowa\" OR \"trening\" OR \"biegaczy\")".into();
    }
    // Quote individual terms to avoid interpreting a user topic as search operators.
    topic
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_topics_reject_general_sport_and_accept_specific_subjects() {
        let mut item = crate::news::tests::fixture()[0].clone();
        item.description_html = None;
        for title in [
            "Malezja spotyka się z krytyką Indonezji",
            "Polska Zbrojna - Polska Zbrojna",
            "SIŁOWNIA OPEN W DNIU 11.11.2026 NIECZYNNA - UMCS",
            "As Sportu Interii i Polsatu Sport 2026. Poznaj zasady plebiscytu",
        ] {
            item.title = title.into();
            assert!(!matches("Wspinaczka sportowa", &item));
            assert!(!matches("Dieta sportowa", &item));
        }
        for title in [
            "Powstaje jedna z najwyższych ścianek wspinaczkowych w Polsce",
            "Mistrzostwa Polski w boulderingu",
            "Wspinaczka sportowa: wyniki finału",
        ] {
            item.title = title.into();
            assert!(matches("Wspinaczka sportowa", &item));
        }
        item.title = "Żywienie sportowców: białko po treningu".into();
        assert!(matches("Dieta sportowa", &item));
        assert!(!matches("Wspinaczka sportowa", &item));
        item.title = "Wybory do samorządu - Dieta Sportowa".into();
        item.description_html =
            Some("<a href='https://dieta-sportowa.pl'>Wyniki głosowania</a>".into());
        assert!(!matches("Dieta sportowa", &item));
        item.title = "Trening siłowy dla sportowców".into();
        item.description_html = Some("Ekspert wyjaśnia zasady żywienia przed zawodami.".into());
        assert!(matches("Dieta sportowa", &item));
    }
}
