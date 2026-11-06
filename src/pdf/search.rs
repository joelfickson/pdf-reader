use std::ops::Range;

pub(crate) const MAX_HITS: usize = 5_000;
const CONTEXT_CHARS: usize = 48;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Query {
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    pub glyphs: Range<usize>,
    pub before: String,
    pub matched: String,
    pub after: String,
}

struct Unit {
    value: char,
    glyph: usize,
}

fn skipped(value: char) -> bool {
    matches!(value, '\r' | '\0' | '\u{2}' | '\u{ad}' | '\u{feff}')
}

fn units(glyphs: &[char], match_case: bool) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::with_capacity(glyphs.len());
    for (glyph, &value) in glyphs.iter().enumerate() {
        if skipped(value) {
            continue;
        }
        if value.is_whitespace() {
            if units.last().is_some_and(|unit| unit.value != ' ') {
                units.push(Unit { value: ' ', glyph });
            }
            continue;
        }
        if value.is_control() {
            continue;
        }
        if match_case {
            units.push(Unit { value, glyph });
        } else {
            units.extend(value.to_lowercase().map(|value| Unit { value, glyph }));
        }
    }
    units
}

fn normalized_query(query: &Query) -> Vec<char> {
    let characters: Vec<char> = query.text.chars().collect();
    let mut folded: Vec<char> = units(&characters, query.match_case)
        .into_iter()
        .map(|unit| unit.value)
        .collect();
    while folded.last() == Some(&' ') {
        folded.pop();
    }
    folded
}

fn display(glyphs: &[char], range: Range<usize>) -> String {
    let mut text = String::new();
    for &value in &glyphs[range] {
        if skipped(value) {
            continue;
        }
        if value.is_whitespace() {
            if !text.ends_with(' ') {
                text.push(' ');
            }
        } else if !value.is_control() {
            text.push(value);
        }
    }
    text
}

fn word_character(value: char) -> bool {
    value.is_alphanumeric() || value == '_'
}

pub(crate) fn find(glyphs: &[char], query: &Query, limit: usize) -> Vec<Hit> {
    let needle = normalized_query(query);
    if needle.is_empty() || limit == 0 {
        return Vec::new();
    }
    let haystack = units(glyphs, query.match_case);
    let mut hits = Vec::new();
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        let matches = haystack[start..start + needle.len()]
            .iter()
            .zip(&needle)
            .all(|(unit, value)| unit.value == *value);
        let end = start + needle.len();
        let bounded = !query.whole_word
            || ((start == 0 || !word_character(haystack[start - 1].value))
                && (end == haystack.len() || !word_character(haystack[end].value)));
        if !(matches && bounded) {
            start += 1;
            continue;
        }
        let first = haystack[start].glyph;
        let last = haystack[end - 1].glyph + 1;
        let before_start = first.saturating_sub(CONTEXT_CHARS);
        let after_end = (last + CONTEXT_CHARS).min(glyphs.len());
        hits.push(Hit {
            glyphs: first..last,
            before: display(glyphs, before_start..first)
                .trim_start()
                .to_string(),
            matched: display(glyphs, first..last),
            after: display(glyphs, last..after_end).trim_end().to_string(),
        });
        if hits.len() >= limit {
            break;
        }
        start = end;
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    fn query(text: &str) -> Query {
        Query {
            text: text.into(),
            match_case: false,
            whole_word: false,
        }
    }

    #[test]
    fn matches_across_pdfium_line_breaks() {
        let page = chars("They open the door, then step\r\naside. They give us");
        let hits = find(&page, &query("step aside"), MAX_HITS);
        assert_eq!(hits.len(), 1);
        let start = page.iter().position(|c| *c == 's').unwrap();
        assert_eq!(
            hits[0].glyphs,
            start..start + "step\r\naside".chars().count()
        );
        assert_eq!(hits[0].matched, "step aside");
    }

    #[test]
    fn folds_unicode_case_by_default_and_respects_match_case() {
        let page = chars("Café CAFÉ café Straße");
        assert_eq!(find(&page, &query("CAFÉ"), MAX_HITS).len(), 3);
        let exact = Query {
            match_case: true,
            ..query("CAFÉ")
        };
        let hits = find(&page, &exact, MAX_HITS);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].glyphs, 5..9);
        assert_eq!(find(&page, &query("straße"), MAX_HITS).len(), 1);
    }

    #[test]
    fn whole_word_rejects_matches_inside_words() {
        let page = chars("page pages paged, page.");
        let whole = Query {
            whole_word: true,
            ..query("page")
        };
        let hits = find(&page, &whole, MAX_HITS);
        assert_eq!(
            hits.iter().map(|hit| hit.glyphs.start).collect::<Vec<_>>(),
            [0, 18]
        );
        assert_eq!(find(&page, &query("page"), MAX_HITS).len(), 4);
    }

    #[test]
    fn matches_do_not_overlap_and_respect_the_limit() {
        let page = chars("aaaa");
        assert_eq!(find(&page, &query("aa"), MAX_HITS).len(), 2);
        assert_eq!(find(&page, &query("a"), 3).len(), 3);
    }

    #[test]
    fn blank_and_whitespace_queries_match_nothing() {
        let page = chars("some text");
        assert!(find(&page, &query(""), MAX_HITS).is_empty());
        assert!(find(&page, &query("   "), MAX_HITS).is_empty());
        assert_eq!(find(&page, &query("  some   text "), MAX_HITS).len(), 1);
    }

    #[test]
    fn context_is_bounded_and_collapses_whitespace() {
        let filler = "word ".repeat(40);
        let page = chars(&format!("{filler}needle\r\n{filler}"));
        let hit = &find(&page, &query("needle"), MAX_HITS)[0];
        assert!(hit.before.chars().count() <= CONTEXT_CHARS);
        assert!(hit.after.chars().count() <= CONTEXT_CHARS);
        assert!(!hit.after.contains('\r'));
        assert!(hit.before.ends_with("word "));
        assert!(hit.after.starts_with(" word"));
    }
}
