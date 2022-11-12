use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};
use lingua::Language::{Chinese, English};

pub fn is_chinese(sentence: &str) -> bool {
    lazy_static! {
        static ref LANGS: Vec<lingua::Language> = vec![Chinese, English];
        // let mut languages = Vec::with_capacity();
        // languages.push(Chinese);
        static ref DETECTOR: LanguageDetector = LanguageDetectorBuilder::
            from_languages(&LANGS).build();
    }
    let detected_language: Option<Language> = DETECTOR.detect_language_of(sentence);
    match detected_language {
        Some(x) => x == Chinese,
        None => false
    }
}
#[cfg(test)]
mod test_language_detect {
    #[test]
    fn zh() {
        use crate::language_detect::is_chinese;
        assert_eq!(is_chinese("李华"), true);
        assert_eq!(is_chinese("rust"), false);
        assert_eq!(is_chinese("Это статья ."), false);
    }
}
// assert_eq!(detected_language, Some(English));