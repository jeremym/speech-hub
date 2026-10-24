//! Text preparation shared by every entry point: normalization now, pronunciation rules in M5.

/// Maximum characters per synthesis chunk. Keeps first-audio latency low and lets Stop feel instant.
const MAX_CHUNK_CHARS: usize = 320;
/// Chunks shorter than this get merged into the next one so we don't synthesize "Hi." alone.
const MIN_CHUNK_CHARS: usize = 24;

/// Whitespace cleanup plus light Markdown/URL cleanup. Never changes what the user sees in the
/// Reader; only what reaches the engine.
pub fn normalize(text: &str) -> String {
    let text = strip_markup(text);
    let text = text.as_str();
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

/// Remove things nobody wants read aloud: Markdown syntax, code fences, raw URLs (replaced by
/// their host), image markup, table pipes. Conservative: when unsure, leave the text alone.
pub fn strip_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        // Horizontal rules → paragraph break.
        if !trimmed.is_empty() && trimmed.chars().all(|c| c == '-' || c == '*' || c == '_' || c == ' ') && trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3 {
            out.push('\n');
            continue;
        }
        let mut l = trimmed.to_string();
        // Headings: drop the hashes, keep the words as their own sentence.
        if let Some(rest) = l.strip_prefix('#') {
            let rest = rest.trim_start_matches('#').trim();
            l = if rest.ends_with(['.', '!', '?', ':']) { rest.to_string() } else { format!("{rest}.") };
        }
        // Block quotes and list bullets.
        l = l.trim_start_matches('>').trim_start().to_string();
        for bullet in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ "] {
            if let Some(rest) = l.strip_prefix(bullet) {
                l = rest.to_string();
                break;
            }
        }
        if let Some(idx) = l.find(". ") {
            if idx <= 3 && l[..idx].chars().all(|c| c.is_ascii_digit()) {
                l = l[idx + 2..].to_string();
            }
        }
        // Tables: drop separator rows, turn pipes into pauses.
        if l.starts_with('|') {
            if l.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ') {
                continue;
            }
            l = l.trim_matches('|').split('|').map(|c| c.trim()).filter(|c| !c.is_empty()).collect::<Vec<_>>().join(", ");
        }
        out.push_str(&strip_inline(&l));
        out.push('\n');
    }
    out
}

fn strip_inline(line: &str) -> String {
    let mut s = line.to_string();
    // Images ![alt](src) → alt ; links [text](url) → text
    s = replace_bracket_link(&s, true);
    s = replace_bracket_link(&s, false);
    // Raw URLs → host name.
    let mut out = String::with_capacity(s.len());
    for word in s.split(' ') {
        if word.starts_with("http://") || word.starts_with("https://") {
            let host = word.split("://").nth(1).and_then(|r| r.split(['/', '?', '#']).next()).unwrap_or("link");
            out.push_str(host.trim_start_matches("www."));
        } else {
            out.push_str(word);
        }
        out.push(' ');
    }
    let s = out.trim_end().to_string();
    // Emphasis and code markers. Only strip when they wrap something (avoid eating "2 * 3").
    let mut s = s.replace("**", "").replace("__", "");
    s = s.replace('`', "");
    if s.matches('*').count() >= 2 && !s.contains(" * ") {
        s = s.replace('*', "");
    }
    s = s.replace("~~", "");
    s
}

fn replace_bracket_link(s: &str, image: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let opener = if image { "![" } else { "[" };
    while let Some(start) = rest.find(opener) {
        let after = &rest[start + opener.len()..];
        if let Some(close) = after.find("](") {
            if let Some(end) = after[close + 2..].find(')') {
                out.push_str(&rest[..start]);
                out.push_str(&after[..close]);
                rest = &after[close + 2 + end + 1..];
                continue;
            }
        }
        out.push_str(&rest[..start + opener.len()]);
        rest = after;
    }
    out.push_str(rest);
    out
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
    fn markdown_is_read_as_prose() {
        let md = "# Release notes\n\n- **Fast**: see [the docs](https://example.com/docs) and https://www.rust-lang.org/learn\n- Use `cargo build`\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n\n> quoted *text*";
        let n = normalize(md);
        assert!(n.starts_with("Release notes."), "{n}");
        assert!(n.contains("Fast: see the docs and rust-lang.org"), "{n}");
        assert!(n.contains("Use cargo build"), "{n}");
        assert!(n.contains("1, 2"), "{n}");
        assert!(n.contains("quoted text"), "{n}");
        assert!(!n.contains('#') && !n.contains('|') && !n.contains("**"), "{n}");
    }

    #[test]
    fn math_asterisks_survive() {
        assert_eq!(normalize("2 * 3 = 6"), "2 * 3 = 6");
    }

    #[test]
    fn empty_is_empty() {
        assert!(segment("   \n ").is_empty());
    }
}
