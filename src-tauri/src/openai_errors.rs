//! Safe diagnostics: never forward raw API messages (401 messages may echo credentials).
use serde_json::Value;
use std::io::Read;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub category: &'static str,
    pub http_status: Option<u16>,
    pub code: Option<String>,
    pub error_type: Option<String>,
    pub request_id: Option<String>,
    pub retry_after_seconds: Option<u32>,
    pub description: &'static str,
    pub action: &'static str,
}
impl ApiError {
    pub fn user_message(&self) -> String {
        let mut text = format!("OpenAI: {} {}", self.description, self.action);
        let mut details = Vec::new();
        if let Some(status) = self.http_status {
            details.push(format!("HTTP {status}"));
        }
        if let Some(code) = &self.code {
            details.push(format!("kod: {code}"));
        }
        if let Some(kind) = &self.error_type {
            details.push(format!("typ: {kind}"));
        }
        if let Some(id) = &self.request_id {
            details.push(format!("request-id: {id}"));
        }
        if let Some(seconds) = self.retry_after_seconds {
            details.push(format!("Retry-After: {seconds} s"));
        }
        if !details.is_empty() {
            text.push_str(&format!(" [{}]", details.join(", ")));
        }
        text
    }
}
pub fn classify(
    status: Option<u16>,
    body: &Value,
    request_id: Option<&str>,
    retry: Option<&str>,
) -> ApiError {
    let error = &body["error"];
    let raw_code = error["code"].as_str().unwrap_or("");
    let raw_type = error["type"].as_str().unwrap_or("");
    let known_codes = [
        "credit_balance_exhausted",
        "organization_usage_limit_exceeded",
        "organization_spend_limit_exceeded",
        "project_spend_limit_exceeded",
        "project_budget_exceeded",
        "billing_hard_limit_reached",
        "billing_not_active",
        "insufficient_quota",
        "rate_limit_exceeded",
        "slow_down",
        "invalid_api_key",
        "model_not_found",
        "permission_denied",
        "context_length_exceeded",
        "invalid_request_error",
        "server_is_overloaded",
    ];
    let known_types = [
        "insufficient_quota",
        "rate_limit_error",
        "invalid_request_error",
        "authentication_error",
        "permission_error",
        "server_error",
        "service_unavailable_error",
    ];
    let (category, description, action) = match raw_code {
        "credit_balance_exhausted" => (
            "credits_exhausted",
            "Wyczerpano kredyty API.",
            "Sprawdź saldo API i rozliczenia właściwej organizacji. Czekanie ani ponawianie nie przywróci środków.",
        ),
        "organization_usage_limit_exceeded" => (
            "usage_limit",
            "Organizacja osiągnęła limit wykorzystania przyznany przez OpenAI.",
            "Sprawdź zatwierdzony limit w ustawieniach organizacji; może być potrzebny wniosek o jego zwiększenie. To osobny limit od ustawionego budżetu i RPM/TPM.",
        ),
        "organization_spend_limit_exceeded" | "billing_hard_limit_reached" => (
            "organization_spend",
            "Osiągnięto ustawiony limit wydatków organizacji.",
            "Sprawdź limit wydatków organizacji; dostęp wymaga zmiany limitu lub jego resetu. Samo czekanie kilku sekund nie pomoże.",
        ),
        "project_spend_limit_exceeded" | "project_budget_exceeded" => (
            "project_budget",
            "Osiągnięto limit wydatków projektu.",
            "Sprawdź projekt przypisany do klucza oraz jego limity.",
        ),
        "billing_not_active" => (
            "billing_inactive",
            "Rozliczenia API nie są aktywne.",
            "Sprawdź konfigurację Billing organizacji używanej przez klucz.",
        ),
        "invalid_api_key" => (
            "invalid_key",
            "Klucz API jest nieprawidłowy, wygasł lub został unieważniony.",
            "Zapisz aktualny klucz w Windows Credential Manager.",
        ),
        "model_not_found" => (
            "model_access",
            "Model nie istnieje lub nie jest dostępny dla tego klucza.",
            "Sprawdź identyfikator modelu i dostęp projektu do tego modelu.",
        ),
        "context_length_exceeded" => (
            "context_limit",
            "Materiały przekraczają kontekst modelu.",
            "Zmniejsz liczbę lub długość materiałów; doładowanie konta nie rozwiąże tego błędu.",
        ),
        _ if raw_code == "insufficient_quota" || raw_type == "insufficient_quota" => (
            "quota",
            "Brak dostępnej kwoty API (środki lub limit wykorzystania).",
            "Sprawdź saldo, limity i właściwą organizację/projekt. Subskrypcja ChatGPT ma osobne rozliczenia. Ten kod nie rozstrzyga sam, czy skończyły się kredyty, czy osiągnięto limit.",
        ),
        "slow_down" => (
            "ramp_limit",
            "Tempo wysyłania żądań wzrosło zbyt szybko.",
            "Odczekaj zgodnie z Retry-After i zwiększaj ruch stopniowo. Ten limit może wystąpić nawet poniżej RPM/TPM. Nie ponowiono żądania automatycznie.",
        ),
        _ if raw_code == "rate_limit_exceeded" || raw_type == "rate_limit_error" => (
            "rate_limit",
            "Przekroczono chwilowy limit zapytań lub tokenów.",
            "Odczekaj zgodnie z Retry-After i sprawdź limity RPM/TPM modelu. Aplikacja nie ponawia płatnego żądania automatycznie.",
        ),
        _ if status == Some(401) => (
            "authentication",
            "Nie udało się uwierzytelnić żądania.",
            "Sprawdź klucz oraz ograniczenia projektu, np. dozwolone adresy IP. Nie każdy HTTP 401 oznacza błędny tekst klucza.",
        ),
        "permission_denied" => (
            "permissions",
            "Klucz nie ma uprawnień do tej operacji.",
            "Sprawdź uprawnienia klucza, projektu i modelu.",
        ),
        _ if status == Some(403) => (
            "permissions",
            "Brak uprawnień do tego endpointu lub zasobu.",
            "Sprawdź ograniczenia klucza i projektu. Sam kod 403 nie dowodzi, że klucz jest nieważny.",
        ),
        _ if status == Some(429) => (
            "unknown_429",
            "Żądanie odrzucono kodem 429, bez rozpoznanego szczegółowego kodu.",
            "Sprawdź zarówno Billing, jak i limity RPM/TPM. Nie można jednoznacznie wskazać przyczyny z tej odpowiedzi.",
        ),
        _ if status.is_some_and(|s| s >= 500) || raw_code == "server_is_overloaded" => (
            "service",
            "Usługa lub model są chwilowo niedostępne.",
            "Spróbuj później; nie wykonano automatycznej płatnej ponownej próby.",
        ),
        _ => (
            "request",
            "Żądanie zostało odrzucone.",
            "Sprawdź model, obsługiwane parametry i uprawnienia. Zachowaj request-id do diagnostyki.",
        ),
    };
    ApiError {
        category,
        http_status: status,
        code: known_codes.contains(&raw_code).then(|| raw_code.into()),
        error_type: known_types.contains(&raw_type).then(|| raw_type.into()),
        request_id: request_id
            .filter(|id| {
                id.starts_with("req_")
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    && !id.contains("sk-")
            })
            .map(str::to_owned),
        retry_after_seconds: retry.and_then(|r| r.parse::<u32>().ok()),
        description,
        action,
    }
}
pub fn response_error(response: reqwest::blocking::Response) -> ApiError {
    let status = response.status().as_u16();
    let request = response
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let retry = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    let _ = response.take(64 * 1024).read_to_end(&mut bytes);
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    classify(Some(status), &body, request.as_deref(), retry.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn distinguishes_quota_credits_rate_and_authentication() {
        for (status, code, kind, category) in [
            (429, "insufficient_quota", "insufficient_quota", "quota"),
            (
                429,
                "credit_balance_exhausted",
                "insufficient_quota",
                "credits_exhausted",
            ),
            (
                429,
                "organization_usage_limit_exceeded",
                "insufficient_quota",
                "usage_limit",
            ),
            (429, "rate_limit_exceeded", "rate_limit_error", "rate_limit"),
            (
                401,
                "invalid_api_key",
                "invalid_request_error",
                "invalid_key",
            ),
            (
                404,
                "model_not_found",
                "invalid_request_error",
                "model_access",
            ),
        ] {
            assert_eq!(
                classify(
                    Some(status),
                    &json!({"error":{"code":code,"type":kind}}),
                    None,
                    None
                )
                .category,
                category
            );
        }
    }
    #[test]
    fn never_forwards_raw_messages_or_unknown_secret_shaped_fields() {
        let body = json!({"error":{"message":"Incorrect API key: sk-fake-sensitive","code":"sk-fake-sensitive","type":"sk-fake-sensitive"}});
        let error = classify(Some(401), &body, Some("sk-fake-sensitive"), Some("bad"));
        let serialized = serde_json::to_string(&error).unwrap();
        assert!(!serialized.contains("sk-fake-sensitive"));
        assert!(!error.user_message().contains("Incorrect API key"));
        let error = classify(Some(429), &Value::Null, Some("req_test-123"), Some("25"));
        assert_eq!(error.category, "unknown_429");
        assert!(error.user_message().contains("Retry-After: 25 s"));
    }
    #[test]
    fn billing_specific_codes_take_precedence_over_legacy_quota_type() {
        for (code, category) in [
            ("organization_spend_limit_exceeded", "organization_spend"),
            ("project_spend_limit_exceeded", "project_budget"),
            ("organization_usage_limit_exceeded", "usage_limit"),
            ("credit_balance_exhausted", "credits_exhausted"),
        ] {
            let error = classify(
                Some(429),
                &json!({"error":{"code":code,"type":"insufficient_quota"}}),
                None,
                None,
            );
            assert_eq!(error.category, category);
            assert_eq!(error.code.as_deref(), Some(code));
        }
        assert_eq!(
            classify(
                Some(429),
                &json!({"error":{"code":"slow_down","type":"rate_limit_error"}}),
                None,
                None
            )
            .category,
            "ramp_limit"
        );
    }
}
