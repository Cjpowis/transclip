//! Which language the person using Transclip reads.

/// This computer's language as a code like "fr" (from "fr-FR"), used as
/// the caption language to look for first. English when it can't be read.
pub fn system_lang() -> String {
    sys_locale::get_locale()
        .as_deref()
        .and_then(lang_code)
        .unwrap_or_else(|| "en".to_string())
}

/// The language part of a locale: "fr" from "fr-FR", "fr_FR.UTF-8" or "fr".
/// "C" and "POSIX", which mean no language is set, give nothing.
fn lang_code(locale: &str) -> Option<String> {
    let code = locale
        .split(['-', '_', '.', '@'])
        .next()?
        .to_ascii_lowercase();
    let letters = code.chars().all(|c| c.is_ascii_alphabetic());
    ((2..=3).contains(&code.len()) && letters).then_some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_language_part_of_a_locale() {
        assert_eq!(lang_code("fr-FR").as_deref(), Some("fr"));
        assert_eq!(lang_code("en-GB").as_deref(), Some("en"));
        assert_eq!(lang_code("de_DE.UTF-8").as_deref(), Some("de"));
        assert_eq!(lang_code("zh-Hans-CN").as_deref(), Some("zh"));
        assert_eq!(lang_code("FIL").as_deref(), Some("fil"));
        assert_eq!(lang_code("C"), None);
        assert_eq!(lang_code("POSIX"), None);
        assert_eq!(lang_code(""), None);
    }
}
