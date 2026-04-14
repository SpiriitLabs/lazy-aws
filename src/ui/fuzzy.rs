/// Returns a score if all characters of `pattern` appear in `text` in order
/// (case-insensitive). Returns `None` if there is no match.
///
/// Scoring:
/// - +1 per matched character
/// - +5 if the match is consecutive to the previous one
/// - +10 if the match is at a word boundary (after `/`, `-`, `_`, `.` or start of string)
pub fn fuzzy_match(text: &str, pattern: &str) -> Option<i32> {
    if pattern.is_empty() {
        return Some(0);
    }

    let text_lower: Vec<char> = text.to_lowercase().chars().collect();
    let pattern_lower: Vec<char> = pattern.to_lowercase().chars().collect();

    let mut score: i32 = 0;
    let mut pi = 0;
    let mut last_match: Option<usize> = None;

    for (ti, &tc) in text_lower.iter().enumerate() {
        if pi < pattern_lower.len() && tc == pattern_lower[pi] {
            // Bonus: consecutive match
            if let Some(prev) = last_match {
                if ti == prev + 1 {
                    score += 5;
                }
            }
            // Bonus: word boundary
            if ti == 0
                || matches!(
                    text_lower.get(ti.wrapping_sub(1)),
                    Some('/' | '-' | '_' | '.' | ' ')
                )
            {
                score += 10;
            }
            last_match = Some(ti);
            pi += 1;
            score += 1;
        }
    }

    if pi == pattern_lower.len() {
        Some(score)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pattern_matches_everything() {
        assert_eq!(fuzzy_match("anything", ""), Some(0));
        assert_eq!(fuzzy_match("", ""), Some(0));
    }

    #[test]
    fn exact_match() {
        let score = fuzzy_match("logs", "logs");
        assert!(score.is_some());
        assert!(score.unwrap() > 0);
    }

    #[test]
    fn substring_match() {
        assert!(fuzzy_match("my-logs-folder", "logs").is_some());
    }

    #[test]
    fn fuzzy_chars_in_order() {
        // "lg" matches "logs" (l...g... no, l then g: l-o-g => l at 0, g at 2)
        assert!(fuzzy_match("logs", "lg").is_some());
        // "abc" in "aXbXc"
        assert!(fuzzy_match("aXbXc", "abc").is_some());
    }

    #[test]
    fn no_match_when_chars_missing() {
        assert!(fuzzy_match("logs", "xyz").is_none());
        assert!(fuzzy_match("abc", "abcd").is_none());
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_match("MyLogFile", "mylog").is_some());
        assert!(fuzzy_match("mylogfile", "MYLOG").is_some());
    }

    #[test]
    fn word_boundary_bonus() {
        // "l" at word boundary scores higher than "l" mid-word
        let boundary = fuzzy_match("my-logs", "l").unwrap();
        let mid_word = fuzzy_match("mylegs", "l").unwrap();
        assert!(boundary > mid_word);
    }

    #[test]
    fn consecutive_bonus() {
        // "log" consecutive in "logs" should score higher than "lxxoxxg" spread out
        let consecutive = fuzzy_match("logs", "log").unwrap();
        let spread = fuzzy_match("lxxoxxg", "log").unwrap();
        assert!(consecutive > spread);
    }

    #[test]
    fn empty_text_no_match() {
        assert!(fuzzy_match("", "a").is_none());
    }
}
