//! Text preparation shared by every entry point: normalization now, pronunciation rules in M5.

/// Maximum characters per synthesis chunk. Keeps first-audio latency low and lets Stop feel instant.
const MAX_CHUNK_CHARS: usize = 320;
/// Chunks shorter than this get merged into the next one so we don't synthesize "Hi." alone.
const MIN_CHUNK_CHARS: usize = 24;

/// Whitespace cleanup. Never changes what the user sees in the Reader; only what reaches the engine.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    let mut newlines = 0u8;
    for ch in text.chars() {
        match ch {
            '\r' => {}
            '\n' => {
                newlines = newlines.saturating_add(1);
            }
            c if c.is_whitespace() => {
                if newlines == 0 && !last_space && !out.is_empty() {
                    out.push(' ');
                    last_space = true;
                }
            }
            c => {
                if newlines >= 2 && !out.is_empty() {
                    out.push_str("\n\n");
                } else if newlines == 1 && !out.is_empty() {
                    out.push(' ');
                }
                newlines = 0;
                out.push(c);
                last_space = false;
            }
        }
    }
    out.trim().to_string()
}

/// Sentence-aware segmentation. Returns non-empty chunks in reading order.
pub fn segment(text: &str) -> Vec<String> {
    let text = normalize(text);
    if text.is_empty() {
        return vec![];
    }
    let mut sentences: Vec<String> = Vec::new();
    for paragraph in text.split("\n\n") {
        let mut current = String::new();
        let chars: Vec<char> = paragraph.chars().collect();
        let n = chars.len();
        let mut i = 0;
        while i < n {
            let c = chars[i];
            current.push(c);
            let is_terminal = matches!(c, '.' | '!' | '?' | '。' | '！' | '？');
            if is_terminal {
                // Swallow closing quotes/brackets right after the terminator.
                let mut j = i + 1;
                while j < n && matches!(chars[j], '"' | '\'' | '”' | '’' | ')' | ']' | '»') {
                    current.push(chars[j]);
                    j += 1;
                }
                let at_boundary = j >= n || chars[j].is_whitespace();
                let looks_like_abbrev = c == '.' && is_abbreviation_tail(&current);
                if at_boundary && !looks_like_abbrev {
                    push_sentence(&mut sentences, std::mem::take(&mut current));
                }
                i = j;
                continue;
            }
            i += 1;
        }
        push_sentence(&mut sentences, current);
    }
    // Merge tiny fragments forward, split oversized ones.
    let mut merged: Vec<String> = Vec::new();
    for s in sentences {
        if let Some(last) = merged.last_mut() {
            if last.chars().count() < MIN_CHUNK_CHARS {
                last.push(' ');
                last.push_str(&s);
                continue;
            }
        }
        merged.push(s);
    }
    merged.into_iter().flat_map(split_long).filter(|s| !s.trim().is_empty()).collect()
}

fn push_sentence(out: &mut Vec<String>, s: String) {
    let t = s.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
}

fn is_abbreviation_tail(s: &str) -> bool {
    let word = s.trim_end_matches('.').rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
    let lower = word.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "mr" | "mrs" | "ms" | "dr" | "prof" | "sr" | "jr" | "st" | "vs" | "etc" | "e.g" | "i.e" | "inc" | "ltd" | "no"
    ) || (word.len() == 1 && word.chars().all(|c| c.is_ascii_uppercase()))
}

fn split_long(s: String) -> Vec<String> {
    if s.chars().count() <= MAX_CHUNK_CHARS {
        return vec![s];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in s.split_whitespace() {
        if !current.is_empty() && current.chars().count() + word.chars().count() + 1 > MAX_CHUNK_CHARS {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        // Prefer breaking after clause punctuation once we're past half the budget.
        if current.chars().count() > MAX_CHUNK_CHARS / 2 && word.ends_with([',', ';', ':']) {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        let v = segment("Hello there, friend. This is the second sentence! And a third one? Yes indeed it is.");
        // Short sentences (< MIN_CHUNK_CHARS) merge forward so we never synthesize tiny fragments.
        assert_eq!(v, vec!["Hello there, friend. This is the second sentence!", "And a third one? Yes indeed it is."]);
        let v = segment("This sentence is comfortably long enough. So is this second sentence here. And this third.");
        assert_eq!(v.len(), 3, "{v:?}");
    }

    #[test]
    fn keeps_abbreviations() {
        let v = segment("Dr. Smith went to Washington D.C. yesterday. He stayed for a long time afterwards.");
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].starts_with("Dr. Smith"));
    }

    #[test]
    fn paragraphs_are_boundaries() {
        let v = segment("First paragraph without terminal punctuation\n\nSecond paragraph here with more words");
        assert_eq!(v.len(), 2, "{v:?}");
    }

    #[test]
    fn long_text_is_bounded() {
        let long = "word ".repeat(400);
        for chunk in segment(&long) {
            assert!(chunk.chars().count() <= MAX_CHUNK_CHARS + 1);
        }
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("a  \t b\nc\n\n\nd"), "a b c\n\nd");
    }

    #[test]
    fn empty_is_empty() {
        assert!(segment("   \n ").is_empty());
    }
}
