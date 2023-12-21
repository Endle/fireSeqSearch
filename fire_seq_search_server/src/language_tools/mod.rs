pub mod tokenizer;
mod cn_stopwords;

use std::collections::HashSet;
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



/// ```
/// let l = fire_seq_search_server::language_tools::generate_stopwords_list();
/// assert!(l.contains("the"));
/// assert!(!l.contains("thex"));
/// ```
pub fn generate_stopwords_list() -> HashSet<String> {
    use stopwords::Stopwords;
    let mut nltk: std::collections::HashSet<&str> = stopwords::NLTK::stopwords(stopwords::Language::English).unwrap().iter().cloned().collect();
    nltk.insert("span");
    nltk.insert("class");
    nltk.insert("fireSeqSearchHighlight");

    nltk.insert("theorem");
    nltk.insert("-");

    nltk.insert("view");


    let mut nltk: HashSet<String> = nltk.iter().map(|&s|s.into()).collect();

    for c in 'a'..='z' {
        nltk.insert(String::from(c));
    }
    // To Improve: I should be aware about the upper/lower case for terms. -Zhenbo Li 2023-Jan-19
    for c in 'A'..='Z' {
        nltk.insert(String::from(c));
    }

    for c in '0'..='9' {
        nltk.insert(String::from(c));
    }


    let words = stop_words::get(stop_words::LANGUAGE::English);
    for w in words {
        nltk.insert(w);
    }
    let words = stop_words::get(stop_words::LANGUAGE::Chinese);
    for w in words {
        nltk.insert(w);
    }
    for c in ['的', '有'] {
        nltk.insert(String::from(c));
    }

    for s in crate::language_tools::cn_stopwords::cn_stopwords_list() {
        nltk.insert(String::from(s));
    }
    for s in crate::language_tools::cn_stopwords::cn_hit_stopword_list() {
        nltk.insert(String::from(s));
    }

    nltk
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
