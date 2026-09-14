//! Normalisation de texte pour la recherche, les empreintes d'import et les tris.

use std::cmp::Ordering;
use unicode_normalization::UnicodeNormalization;

fn is_combining_diacritic(character: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&character)
}

/// Minuscules sans accents, comme `normalizeSearchValue` en 1.x.
pub fn normalize_search(value: &str) -> String {
    value
        .to_lowercase()
        .nfd()
        .filter(|character| !is_combining_diacritic(*character))
        .collect()
}

/// Découpe une recherche en mots normalisés.
pub fn search_tokens(query: &str) -> Vec<String> {
    normalize_search(query).split_whitespace().map(str::to_owned).collect()
}

/// `trim()` puis réduction des espaces multiples.
pub fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Description normalisée utilisée pour reconnaître une échéance récurrente.
pub fn normalize_description(value: &str) -> String {
    collapse_whitespace(value).to_lowercase()
}

/// Comparaison proche de `localeCompare(_, 'fr')` : accents et casse ignorés d'abord.
pub fn compare_fr(left: &str, right: &str) -> Ordering {
    normalize_search(left)
        .cmp(&normalize_search(right))
        .then_with(|| left.cmp(right))
}

/// Texte agrégé d'une ligne, interrogé mot par mot.
#[derive(Debug, Default, Clone)]
pub struct SearchText {
    content: String,
}

impl SearchText {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: impl AsRef<str>) -> &mut Self {
        self.content.push_str(&normalize_search(value.as_ref()));
        self.content.push(' ');
        self
    }

    pub fn matches(&self, tokens: &[String]) -> bool {
        tokens.iter().all(|token| self.content.contains(token.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_accents_and_case() {
        assert_eq!(normalize_search("Café Énergie"), "cafe energie");
    }

    #[test]
    fn tokens_ignore_extra_spaces() {
        assert_eq!(search_tokens("  Loyer   Août "), vec!["loyer", "aout"]);
    }

    #[test]
    fn search_text_requires_every_token() {
        let mut text = SearchText::new();
        text.push("Courses Carrefour").push("Épargne");
        assert!(text.matches(&search_tokens("carrefour epargne")));
        assert!(!text.matches(&search_tokens("carrefour loyer")));
    }
}
