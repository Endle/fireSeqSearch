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
        use crate::language_tools::is_chinese;
        assert!(is_chinese("李华"));
        assert!(!is_chinese("rust"));
        assert!(!is_chinese("Это статья ."));
    }
}
// assert_eq!(detected_language, Some(English));