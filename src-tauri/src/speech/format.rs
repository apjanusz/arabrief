use sha2::{Digest, Sha256};
pub const RATE: u32 = 24000;
pub const MAX_PCM: usize = 24000 * 2 * 15 * 60;
pub const MODEL: &str = "gpt-4o-mini-tts";
pub const VOICES: &[&str] = &["marin", "cedar"];
pub const INSTRUCTIONS: &str = "Speak exclusively in Polish with a natural, native Polish accent. Use clear, authentic Polish pronunciation without any English or American accent. Sound like a professional Polish radio news presenter: warm, calm, confident and engaging. Maintain a natural conversational rhythm, moderate speaking pace, subtle pauses between sentences and expressive but restrained intonation. Pronounce Polish names, cities and numbers correctly. Avoid robotic delivery, exaggerated emotions and unnatural pauses.";
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn instructions(language: &str) -> &'static str {
    if language == "en" {
        "Speak exclusively in natural, fluent English. Read the supplied script exactly, without adding, omitting or translating content. Sound like a professional radio news presenter: warm, calm, confident and engaging. Maintain a conversational rhythm, moderate pace, subtle pauses and restrained expression. Pronounce names, places and numbers clearly. Avoid robotic delivery and exaggerated emotions."
    } else {
        INSTRUCTIONS
    }
}
pub fn contextual_cache_key(
    text: &str,
    voice: &str,
    settings: &crate::settings::AppSettings,
) -> String {
    hash(
        serde_json::json!([
            "speech-v2-locale",
            cache_key(text, voice),
            settings.broadcast_language(),
            settings.effective_region(),
            instructions(settings.broadcast_language())
        ])
        .to_string()
        .as_bytes(),
    )
}
pub fn cache_key(text: &str, voice: &str) -> String {
    hash(
        serde_json::to_string(&(
            "arabrief-speech-v1",
            MODEL,
            voice,
            RATE,
            800,
            INSTRUCTIONS,
            text,
        ))
        .expect("string tuple")
        .as_bytes(),
    )
}
pub fn normalized(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().collect::<String>()
            } else {
                " ".into()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn verify(script: &str, transcript: &str) -> Result<(), String> {
    if normalized(script).is_empty() || normalized(script) != normalized(transcript) {
        return Err("Transkrypcja Realtime nie odpowiada scenariuszowi. Nie zapisano nagrania i nie ponowiono płatnego żądania.".into());
    }
    Ok(())
}
pub fn segments(text: &str) -> Result<Vec<String>, String> {
    if text.trim().is_empty() || text.len() > 30000 {
        return Err("Scenariusz pusty lub zbyt długi".into());
    }
    let mut remaining = text.trim();
    let mut result = Vec::new();
    while remaining.chars().count() > 800 {
        let limit = remaining.char_indices().nth(800).expect("length checked").0;
        let prefix = &remaining[..limit];
        let boundary = prefix
            .rfind('\n')
            .map(|i| i + 1)
            .or_else(|| {
                prefix
                    .char_indices()
                    .rev()
                    .find(|(i, c)| {
                        ['.', '!', '?'].contains(c)
                            && prefix[*i + c.len_utf8()..].starts_with(char::is_whitespace)
                    })
                    .map(|(i, c)| i + c.len_utf8())
            })
            .or_else(|| prefix.rfind(char::is_whitespace))
            .filter(|i| *i > 0)
            .ok_or("Zbyt długi niepodzielny fragment skryptu")?;
        result.push(remaining[..boundary].trim().to_owned());
        remaining = remaining[boundary..].trim_start();
    }
    if !remaining.trim().is_empty() {
        result.push(remaining.trim().into());
    }
    if result.is_empty() || result.len() > 60 {
        return Err("Zbyt wiele segmentów skryptu".into());
    }
    Ok(result)
}
pub fn wav(pcm: &[u8]) -> Result<Vec<u8>, String> {
    if pcm.is_empty() || !pcm.len().is_multiple_of(2) || pcm.len() > MAX_PCM {
        return Err("Nieprawidłowe lub zbyt długie PCM16".into());
    }
    let size = pcm.len() as u32;
    let mut bytes = Vec::with_capacity(pcm.len() + 44);
    bytes.extend(b"RIFF");
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(RATE.to_le_bytes());
    bytes.extend((RATE * 2).to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    bytes.extend(pcm);
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instructions_and_audio_keys_track_language_and_region() {
        let repo =
            crate::settings::SettingsRepository::open(std::path::Path::new(":memory:")).unwrap();
        let mut settings = repo.load().unwrap();
        settings.ui_language = "pl".into();
        settings.news_region = "US".into();
        let polish = contextual_cache_key("Shared words", "marin", &settings);
        settings.ui_language = "en".into();
        let english = contextual_cache_key("Shared words", "marin", &settings);
        assert_ne!(polish, english);
        settings.news_region = "GB".into();
        assert_ne!(
            english,
            contextual_cache_key("Shared words", "marin", &settings)
        );
        assert!(instructions("en").contains("English"));
        assert!(!instructions("en").contains("exclusively in Polish"));
        assert!(instructions("pl").contains("Polish"));
    }
    #[test]
    fn logical_segments_preserve_polish_text_and_boundaries() {
        let text = "To jest spokojna audycja po polsku. Żółw idzie do źródła.\n\n".repeat(70);
        let parts = segments(&text).unwrap();
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|s| s.chars().count() <= 800));
        assert_eq!(normalized(&text), normalized(&parts.join(" ")));
        assert!(segments("").is_err());
        assert!(segments(&"x".repeat(1000)).is_err());
    }
    #[test]
    fn wav_has_correct_header_and_pcm_even_when_chunks_are_odd() {
        let mut pcm = vec![1];
        pcm.extend([2, 3, 4]);
        let bytes = wav(&pcm).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 40);
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 24000);
        assert_eq!(&bytes[44..], pcm);
        assert!(wav(&[1]).is_err());
        assert!(wav(&[]).is_err());
    }
    #[test]
    fn transcript_check_is_conservative_and_cache_tracks_inputs() {
        verify("Dzień dobry, Polsko!", "dzień dobry polsko").unwrap();
        assert!(verify("To są fakty.", "Dzień dobry. To są fakty.").is_err());
        assert!(verify("W roku 2026", "W roku dwa tysiące dwadzieścia sześć").is_err());
        assert_ne!(cache_key("tekst", "alloy"), cache_key("tekst", "marin"));
        assert_ne!(
            cache_key("tekst", "alloy"),
            cache_key("inny tekst", "alloy")
        );
    }
}
