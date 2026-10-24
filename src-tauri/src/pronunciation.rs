//! Pronunciation Studio: one shared substitution dictionary applied to synthesis input only.
//! "LangGraph" → "Lang Graph" everywhere: selection, CLI, every Source. The text the user sees
//! in the Reader never changes.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    /// The written form to look for.
    pub match_text: String,
    /// What the engine should be given instead.
    pub replacement: String,
    #[serde(default = "yes")]
    pub whole_word: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub created_at: u64,
}

fn yes() -> bool {
    true
}

impl Rule {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(match_text: &str, replacement: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            match_text: match_text.into(),
            replacement: replacement.into(),
            whole_word: true,
            case_sensitive: false,
            enabled: true,
            note: String::new(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        }
    }

    fn sanitize(mut self) -> Result<Self, String> {
        self.match_text = self.match_text.trim().chars().take(200).collect();
        self.replacement = self.replacement.trim().chars().take(400).collect();
        self.note = self.note.chars().take(300).collect();
        if self.match_text.is_empty() {
            return Err("the text to match can't be empty".into());
        }
        if self.id.is_empty() {
            self.id = uuid::Uuid::new_v4().to_string();
        }
        Ok(self)
    }
}

/// Apply rules deterministically: longest match first (so "LangGraph Cloud" beats "LangGraph"),
/// then oldest first. Each rule replaces every occurrence it finds in the current text.
pub fn apply(text: &str, rules: &[Rule]) -> String {
    let mut active: Vec<&Rule> = rules.iter().filter(|r| r.enabled && !r.match_text.is_empty()).collect();
    active.sort_by(|a, b| b.match_text.chars().count().cmp(&a.match_text.chars().count()).then(a.created_at.cmp(&b.created_at)));
    let mut current: Vec<char> = text.chars().collect();
    for rule in active {
        current = replace_all(&current, rule);
    }
    current.into_iter().collect()
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn replace_all(text: &[char], rule: &Rule) -> Vec<char> {
    let needle: Vec<char> = rule.match_text.chars().collect();
    let n = needle.len();
    if n == 0 || text.len() < n {
        return text.to_vec();
    }
    let eq = |a: char, b: char| if rule.case_sensitive { a == b } else { fold(a) == fold(b) };
    let mut out: Vec<char> = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let fits = i + n <= text.len() && (0..n).all(|k| eq(text[i + k], needle[k]));
        if fits {
            let boundary_ok = !rule.whole_word
                || ((i == 0 || !is_word_char(text[i - 1]) || !is_word_char(needle[0]))
                    && (i + n == text.len() || !is_word_char(text[i + n]) || !is_word_char(needle[n - 1])));
            if boundary_ok {
                out.extend(rule.replacement.chars());
                i += n;
                continue;
            }
        }
        out.push(text[i]);
        i += 1;
    }
    out
}

#[derive(Clone)]
pub struct RuleStore {
    path: PathBuf,
    inner: Arc<Mutex<Vec<Rule>>>,
}

pub const CHANGED_EVENT: &str = "pronunciation:changed";

impl RuleStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("pronunciation.json");
        let list = fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Vec<Rule>>(&b).ok()).unwrap_or_default();
        Self { path, inner: Arc::new(Mutex::new(list)) }
    }

    pub fn list(&self) -> Vec<Rule> {
        self.inner.lock().unwrap().clone()
    }

    pub fn apply(&self, text: &str) -> String {
        let rules = self.inner.lock().unwrap();
        if rules.is_empty() {
            return text.to_string();
        }
        apply(text, &rules)
    }

    pub fn upsert(&self, rule: Rule) -> Result<Rule, String> {
        let rule = rule.sanitize()?;
        {
            let mut list = self.inner.lock().unwrap();
            match list.iter_mut().find(|r| r.id == rule.id) {
                Some(slot) => *slot = rule.clone(),
                None => list.push(rule.clone()),
            }
        }
        self.save()?;
        Ok(rule)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.inner.lock().unwrap().retain(|r| r.id != id);
        self.save()
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.list()).unwrap_or_else(|_| "[]".into())
    }

    /// Merge: a rule with the same match text (same case-sensitivity) replaces the existing one.
    pub fn import_json(&self, json: &str) -> Result<usize, String> {
        let incoming: Vec<Rule> = serde_json::from_str(json).map_err(|e| format!("not a pronunciation export: {e}"))?;
        let n = incoming.len();
        for mut r in incoming {
            let existing = self.inner.lock().unwrap().iter().find(|e| e.match_text.eq_ignore_ascii_case(&r.match_text)).map(|e| e.id.clone());
            r.id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            self.upsert(r)?;
        }
        Ok(n)
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(&*self.inner.lock().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_word_case_insensitive_default() {
        let r = vec![Rule::new("LangGraph", "Lang Graph")];
        assert_eq!(apply("I like langgraph. LangGraph! LangGraphs?", &r), "I like Lang Graph. Lang Graph! LangGraphs?");
    }

    #[test]
    fn case_sensitive_and_partial() {
        let mut r = Rule::new("SQL", "sequel");
        r.case_sensitive = true;
        r.whole_word = false;
        assert_eq!(apply("MySQL and sql and SQL", &[r]), "Mysequel and sql and sequel");
    }

    #[test]
    fn longest_match_wins_and_order_is_stable() {
        let mut a = Rule::new("LangGraph", "Lang Graph");
        a.created_at = 1;
        let mut b = Rule::new("LangGraph Cloud", "Lang Graph Cloud service");
        b.created_at = 2;
        assert_eq!(apply("Use LangGraph Cloud or LangGraph.", &[a, b]), "Use Lang Graph Cloud service or Lang Graph.");
    }

    #[test]
    fn disabled_rules_are_skipped_and_unicode_is_fine() {
        let mut off = Rule::new("naïve", "nah-eve");
        off.enabled = false;
        let on = Rule::new("Zürich", "Zoo-rick");
        assert_eq!(apply("naïve Zürich zürich", &[off, on]), "naïve Zoo-rick Zoo-rick");
    }

    #[test]
    fn punctuation_match_without_word_chars() {
        let mut r = Rule::new("->", " goes to ");
        r.whole_word = true;
        assert_eq!(apply("a->b", &[r]), "a goes to b");
    }

    #[test]
    fn store_roundtrip_and_merge() {
        let dir = std::env::temp_dir().join(format!("speechhub-pron-{}", uuid::Uuid::new_v4()));
        let s = RuleStore::load(&dir);
        s.upsert(Rule::new("GPU", "G P U")).unwrap();
        assert_eq!(s.apply("my GPU"), "my G P U");
        let n = s.import_json(&serde_json::to_string(&vec![Rule::new("gpu", "graphics unit")]).unwrap()).unwrap();
        assert_eq!(n, 1);
        assert_eq!(s.list().len(), 1, "same match text merges");
        assert_eq!(s.apply("my GPU"), "my graphics unit");
        let _ = fs::remove_dir_all(dir);
    }
}
