//! Repair whole independent news units. Never invent or splice unsupported claims.
use super::*;
fn repeated(text: &str, previous: &str) -> bool {
    let a = normalized(text);
    let b = normalized(previous);
    if b.contains(&a) {
        return true;
    }
    let words = |s: &str| {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<HashSet<_>>()
    };
    let a = words(&a);
    let b = words(&b);
    let numbers = |set: &HashSet<String>| {
        set.iter()
            .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
            .cloned()
            .collect::<HashSet<_>>()
    };
    a.len() >= 8
        && numbers(&a) == numbers(&b)
        && a.intersection(&b).count() * 100 >= a.len().max(b.len()) * 90
}
fn problem(segment: &Segment, context: &Context) -> Option<&'static str> {
    if segment.text.trim().chars().count() < 12
        || segment.text.len() > 30000
        || segment.evidence.is_empty()
        || segment.evidence.len() > 10
    {
        return Some("invalid_segment");
    }
    for evidence in &segment.evidence {
        let Some(source) = context.sources.iter().find(|s| s.id == evidence.source_id) else {
            return Some("unknown_source");
        };
        if evidence.quote.chars().count() < 12
            || evidence.quote.len() > 1200
            || !(source.title.contains(&evidence.quote)
                || source
                    .description_html
                    .as_ref()
                    .is_some_and(|text| text.contains(&evidence.quote)))
        {
            return Some("unsupported_evidence");
        }
    }
    if context
        .previous
        .iter()
        .flat_map(|old| old.split("\n\n"))
        .any(|old| repeated(&segment.text, old))
    {
        return Some("repeated_narration");
    }
    if segment.evidence.iter().all(|e| {
        context.previous_quotes.contains(&normalized(&e.quote))
            && context
                .previous
                .iter()
                .any(|old| normalized(old).contains(&normalized(&e.quote)))
    }) {
        return Some("repeated_information_without_new_evidence");
    }
    None
}
#[cfg(test)]
fn publication(segment: &Segment, context: &Context) -> i64 {
    segment
        .evidence
        .iter()
        .filter_map(|e| context.sources.iter().find(|s| s.id == e.source_id))
        .filter_map(|s| s.published_at.as_deref())
        .filter_map(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.timestamp())
        .max()
        .unwrap_or(i64::MIN)
}
#[cfg(test)]
pub(super) fn repair(draft: &mut Draft, context: &Context) -> Result<(), String> {
    let original = draft.segments.len();
    let mut kept: Vec<Segment> = Vec::new();
    for (index, mut segment) in std::mem::take(&mut draft.segments).into_iter().enumerate() {
        let reason = problem(&segment, context).or_else(|| {
            kept.iter()
                .any(|old| repeated(&segment.text, &old.text))
                .then_some("duplicate_in_draft")
        });
        if let Some(reason) = reason {
            let sources = segment
                .evidence
                .iter()
                .map(|e| e.source_id.as_str())
                .collect::<Vec<_>>()
                .join(",");
            crate::performance::diagnostic!(
                "[recovery] stage=content segment={index} sources={sources} action=skipped reason={reason}"
            );
            continue;
        }
        if segment.topic.trim().is_empty() || segment.topic.len() > 200 {
            segment.topic = "Wiadomości".into();
        }
        if segment
            .evidence
            .iter()
            .any(|e| context.previous_quotes.contains(&normalized(&e.quote)))
        {
            crate::performance::diagnostic!(
                "[recovery] stage=content segment={index} action=kept reason=shared_quote_is_not_repeated_narration"
            );
        }
        kept.push(segment);
    }
    let order: Vec<_> = kept.iter().map(|s| publication(s, context)).collect();
    kept.sort_by_key(|s| std::cmp::Reverse(publication(s, context)));
    let reordered = order
        != kept
            .iter()
            .map(|s| publication(s, context))
            .collect::<Vec<_>>();
    if reordered {
        crate::performance::diagnostic!(
            "[recovery] stage=content action=reordered reason=publication_date"
        );
    }
    if reordered || kept.len() != original {
        kept.retain(|s| {
            ![
                "jak już wspomniano",
                "poprzedniej wiadomości",
                "wspomnianego wyżej",
            ]
            .iter()
            .any(|phrase| s.text.to_lowercase().contains(phrase))
        });
    }
    let mut words = 0usize;
    let mut bytes = 0usize;
    let max_words = context.settings.broadcast_max_duration_ms as usize * 140 / 60000 * 12 / 10;
    kept.retain(|segment| {
        let size = segment.text.split_whitespace().count();
        if words + size > max_words || bytes + segment.text.len() > 30000 {
            crate::performance::diagnostic!(
                "[recovery] stage=content action=skipped reason=duration_budget"
            );
            false
        } else {
            words += size;
            bytes += segment.text.len();
            true
        }
    });
    let removed = original.saturating_sub(kept.len());
    if kept.is_empty() {
        return Err("Brak bezpiecznych materiałów: wszystkie fragmenty były powtórzone, niespójne lub niepotwierdzone.".into());
    }
    draft.segments = kept;
    if draft.title.trim().is_empty() || draft.title.len() > 300 {
        draft.title = "Przegląd wiadomości AraBrief".into();
    }
    draft
        .limitations
        .retain(|s| !s.trim().is_empty() && s.len() <= 2000);
    draft.limitations.truncate(18);
    if removed > 0 {
        draft.insufficient_material = true;
        draft.limitations.push(format!("Pominięto {removed} fragmentów, które nie przeszły kontroli. Pozostawiono wyłącznie zweryfikowane materiały."));
    }
    crate::performance::diagnostic!(
        "[recovery] stage=content action=prepared segments={} omitted={removed}",
        draft.segments.len()
    );
    validate(draft, context)
}
pub(super) fn validate(draft: &Draft, context: &Context) -> Result<(), String> {
    if draft.segments.is_empty() || draft.title.trim().is_empty() {
        return Err("Brak bezpiecznych materiałów: scenariusz jest pusty.".into());
    }
    for segment in &draft.segments {
        if let Some(reason) = problem(segment, context) {
            return Err(format!(
                "Nieprawidłowy segment «{}»: {reason}",
                segment.topic
            ));
        }
    }
    Ok(())
}
