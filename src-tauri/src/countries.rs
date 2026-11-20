#[derive(serde::Deserialize)]
pub(crate) struct Country {
    pub code: String,
    pub language: String,
    pub hl: String,
}
pub(crate) fn all() -> &'static [Country] {
    static COUNTRIES: std::sync::OnceLock<Vec<Country>> = std::sync::OnceLock::new();
    COUNTRIES.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/domain/newsCountries.json"))
            .expect("embedded country catalog")
    })
}
pub(crate) fn find(code: &str) -> Option<&'static Country> {
    all().iter().find(|country| country.code == code)
}

#[cfg(test)]
mod tests {
    #[test]
    fn country_catalog_has_unique_codes_and_romanian_edition() {
        let mut codes = std::collections::HashSet::new();
        for country in super::all() {
            assert_eq!(country.code.len(), 2);
            assert!(codes.insert(&country.code));
            assert!(!country.hl.is_empty() && !country.language.is_empty());
        }
        let romania = super::find("RO").unwrap();
        assert_eq!(romania.hl, "ro");
        assert_eq!(romania.language, "ro");
        assert!(super::find("auto").is_none());
    }
}
