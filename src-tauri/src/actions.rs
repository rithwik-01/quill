//! actions.rs — four rewrite actions (PLAN.md §9)
//! Prompts are verbatim from PLAN.md; do not rephrase.
//! Also: request body construction and strip_preamble filter.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action { FixGrammar, Improve, Shorten, Simplify }

pub const FIX_GRAMMAR_PROMPT: &str = "You are a copy editor. Correct spelling, grammar, and punctuation in the user's text.\nPreserve the author's voice, tone, word choice, and formatting exactly.\nDo not rephrase, shorten, or improve style. Do not add or remove content.\nOutput only the corrected text, with no preamble, explanation, or quotation marks.";
pub const IMPROVE_PROMPT: &str = "You are an editor. Rewrite the user's text to be clearer and better structured.\nKeep the author's voice and intent. Keep roughly the same length.\nOutput only the rewritten text, with no preamble, explanation, or quotation marks.";
pub const SHORTEN_PROMPT: &str = "You are an editor. Make the user's text shorter while keeping every substantive point.\nAim for 60-75% of the original length. Keep the author's voice.\nOutput only the shortened text, with no preamble, explanation, or quotation marks.";
pub const SIMPLIFY_PROMPT: &str = "You are an editor. Rewrite the user's text in plain language.\nReplace jargon with everyday words, break up long sentences, use active voice.\nKeep all the original meaning.\nOutput only the rewritten text, with no preamble, explanation, or quotation marks.";
/// Refine chat (popup): applies one user instruction to the current version.
/// Stateless by design — the caller embeds original + current + instruction
/// in a single user message, which small models handle more reliably than
/// multi-turn history.
pub const REFINE_PROMPT: &str = "You are an editor refining a text rewrite.\nYou receive the user's original text, the current revised version, and one instruction.\nApply the instruction to the current version only; preserve everything else.\nOutput only the updated text, with no preamble, explanation, or quotation marks.";

impl Action {
    #[inline] pub fn prompt(&self) -> &'static str {
        match self {
            Action::FixGrammar => FIX_GRAMMAR_PROMPT,
            Action::Improve    => IMPROVE_PROMPT,
            Action::Shorten    => SHORTEN_PROMPT,
            Action::Simplify   => SIMPLIFY_PROMPT,
        }
    }
    #[inline] pub fn as_str(&self) -> &'static str {
        match self {
            Action::FixGrammar => "fix_grammar",
            Action::Improve    => "improve",
            Action::Shorten    => "shorten",
            Action::Simplify   => "simplify",
        }
    }
}

// ---------- request body (PLAN.md §9) ----------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChatMessage { pub role: String, pub content: String }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatOptions { pub temperature: f32, pub num_ctx: u32 }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub think: bool,
    pub options: ChatOptions,
}

/// Build the `/api/chat` body: think:false, stream:false, temp 0.2, num_ctx 8192.
#[inline]
pub fn build_chat_request(model: &str, action: Action, selected_text: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage { role: "system".into(), content: action.prompt().into() },
            ChatMessage { role: "user".into(),   content: selected_text.to_string() },
        ],
        stream: false,
        think: false,
        options: ChatOptions { temperature: 0.2, num_ctx: 8192 },
    }
}

#[inline]
pub fn build_chat_body_json(model: &str, action: Action, selected_text: &str) -> serde_json::Value {
    serde_json::to_value(build_chat_request(model, action, selected_text)).expect("serializable")
}

// ---------- strip_preamble ----------

/// (a) trim, (b) drop leading `^(Here|Sure|Certainly|Corrected|Okay|Of course)\b.*:\s*$`, (c) strip fences/quotes.
pub fn strip_preamble(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.is_empty() { return s; }
    if let Some(rest) = strip_leading_preamble_line(&s) {
        s = rest.trim().to_string();
        if s.is_empty() { return s; }
    }
    s = strip_fences(&s);
    s = strip_symmetric_quotes(&s);
    s.trim().to_string()
}

const PREAMBLE_KEYWORDS: &[&str] = &["Here", "Sure", "Certainly", "Corrected", "Okay", "Of course"];
fn is_word_char(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }

fn strip_leading_preamble_line(s: &str) -> Option<String> {
    let end = s.find('\n').unwrap_or(s.len());
    let first = &s[..end];
    if !first.trim_end().ends_with(':') { return None; }
    let line = first.trim();
    let hit = PREAMBLE_KEYWORDS.iter().find(|kw| {
        if !line.starts_with(*kw) { return false; }
        let n = kw.len();
        if line.len() == n { return true; }
        !is_word_char(line.as_bytes()[n])
    })?;
    let _ = hit;
    let rest = if end == s.len() { s.len() } else { end + 1 };
    Some(s[rest..].to_string())
}

fn strip_fences(s: &str) -> String {
    let t = s.trim();
    if !t.starts_with("```") || !t.ends_with("```") || t.len() < 6 { return s.to_string(); }
    let inner_raw = &t[3..t.len()-3];
    if inner_raw.trim().is_empty() { return String::new(); }
    if let Some(nl) = inner_raw.find('\n') {
        let first = &inner_raw[..nl];
        let first_trim = first.trim();
        // Only treat first line as language tag if it was attached to the opening fence
        // i.e. inner_raw does not start with newline.  "```\nHello" -> first="" -> no tag.
        // "```json\nHello" -> first="json" -> tag.
        if !first_trim.is_empty() && !inner_raw.starts_with('\n') && !inner_raw.starts_with("\r") {
            let is_tag = first_trim.chars().all(|c| c.is_alphanumeric() || matches!(c,'-'|'_'|'+'|'.'));
            if is_tag {
                return inner_raw[nl+1..].trim().to_string();
            }
        }
        return inner_raw.trim().to_string();
    }
    inner_raw.trim().to_string()
}

fn strip_symmetric_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 {
        let (a,b) = (t.as_bytes()[0], t.as_bytes()[t.len()-1]);
        if (a==b'"' && b==b'"') || (a==b'\'' && b==b'\'') {
            return t[1..t.len()-1].trim().to_string();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn prompts_verbatim() {
        assert_eq!(FIX_GRAMMAR_PROMPT, "You are a copy editor. Correct spelling, grammar, and punctuation in the user's text.\nPreserve the author's voice, tone, word choice, and formatting exactly.\nDo not rephrase, shorten, or improve style. Do not add or remove content.\nOutput only the corrected text, with no preamble, explanation, or quotation marks.");
        assert_eq!(IMPROVE_PROMPT, "You are an editor. Rewrite the user's text to be clearer and better structured.\nKeep the author's voice and intent. Keep roughly the same length.\nOutput only the rewritten text, with no preamble, explanation, or quotation marks.");
        assert_eq!(SHORTEN_PROMPT, "You are an editor. Make the user's text shorter while keeping every substantive point.\nAim for 60-75% of the original length. Keep the author's voice.\nOutput only the shortened text, with no preamble, explanation, or quotation marks.");
        assert_eq!(SIMPLIFY_PROMPT, "You are an editor. Rewrite the user's text in plain language.\nReplace jargon with everyday words, break up long sentences, use active voice.\nKeep all the original meaning.\nOutput only the rewritten text, with no preamble, explanation, or quotation marks.");
    }
    #[test] fn dispatch() {
        assert_eq!(Action::FixGrammar.prompt(), FIX_GRAMMAR_PROMPT);
        assert_eq!(Action::Improve.prompt(), IMPROVE_PROMPT);
        assert_eq!(Action::Shorten.prompt(), SHORTEN_PROMPT);
        assert_eq!(Action::Simplify.prompt(), SIMPLIFY_PROMPT);
    }
    #[test] fn chat_shape() {
        let r = build_chat_request("qwen3.5:4b", Action::FixGrammar, "hi");
        assert_eq!(r.model, "qwen3.5:4b"); assert_eq!(r.stream, false); assert_eq!(r.think, false);
        assert_eq!(r.options.temperature, 0.2); assert_eq!(r.options.num_ctx, 8192);
        assert_eq!(r.messages[0].role, "system"); assert_eq!(r.messages[0].content, FIX_GRAMMAR_PROMPT);
        assert_eq!(r.messages[1].content, "hi");
    }
    #[test] fn chat_json_keys() {
        let v = build_chat_body_json("qwen3.5:9b", Action::Simplify, "jargon");
        assert_eq!(v["model"], "qwen3.5:9b"); assert_eq!(v["stream"], false); assert_eq!(v["think"], false);
        assert!((v["options"]["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-6); assert_eq!(v["options"]["num_ctx"], 8192);
        assert!(v["messages"][0]["content"].as_str().unwrap().contains("plain language"));
        assert_eq!(v["messages"][1]["content"], "jargon");
    }
    #[test] fn trim_and_passthrough() {
        assert_eq!(strip_preamble("  hello  "), "hello");
        assert_eq!(strip_preamble("\n\thello world\n\n"), "hello world");
        assert_eq!(strip_preamble(""), ""); assert_eq!(strip_preamble("   \n\t  "), "");
        assert_eq!(strip_preamble("This is the corrected sentence."), "This is the corrected sentence.");
        assert_eq!(strip_preamble("Multiple\nlines\nstay"), "Multiple\nlines\nstay");
    }
    #[test] fn preamble_here_variants() {
        assert_eq!(strip_preamble("Here is the corrected text:\nHello world"), "Hello world");
        assert_eq!(strip_preamble("Here you go:\nHello"), "Hello");
        assert_eq!(strip_preamble("Here is what I did:\nLine1\nLine2"), "Line1\nLine2");
        assert_eq!(strip_preamble("Here is the corrected text:   \nHello"), "Hello");
    }
    #[test] fn preamble_all_keywords() {
        assert_eq!(strip_preamble("Sure, here is the corrected text:\nHello"), "Hello");
        assert_eq!(strip_preamble("Certainly! Here you go:\nHello"), "Hello");
        assert_eq!(strip_preamble("Corrected text:\nHello"), "Hello");
        assert_eq!(strip_preamble("Okay, here is the result:\nHello"), "Hello");
        assert_eq!(strip_preamble("Of course, here it is:\nHello"), "Hello");
    }
    #[test] fn preamble_requires_colon() {
        assert_eq!(strip_preamble("Here is the corrected text\nHello"), "Here is the corrected text\nHello");
        assert_eq!(strip_preamble("Sure thing\nHello"), "Sure thing\nHello");
        assert_eq!(strip_preamble("Here: is something\nHello"), "Here: is something\nHello");
        assert_eq!(strip_preamble("Here is: the text\nHello"), "Here is: the text\nHello");
    }
    #[test] fn preamble_case_and_position() {
        assert_eq!(strip_preamble("here is the corrected text:\nHello"), "here is the corrected text:\nHello");
        assert_eq!(strip_preamble("HERE IS THE TEXT:\nHello"), "HERE IS THE TEXT:\nHello");
        assert_eq!(strip_preamble("Hello\nHere is the corrected text:\nWorld"), "Hello\nHere is the corrected text:\nWorld");
        assert_eq!(strip_preamble("Here is the corrected text:"), "");
        assert_eq!(strip_preamble("Here's the corrected text:\nHello"), "Hello"); // \b after Here -> stripped
        assert_eq!(strip_preamble("Heretic philosophy:\nHello"), "Heretic philosophy:\nHello");
    }
    #[test] fn quotes() {
        assert_eq!(strip_preamble("\"Hello world\""), "Hello world");
        assert_eq!(strip_preamble("  \"Hello world\"  "), "Hello world");
        assert_eq!(strip_preamble("'Hello world'"), "Hello world");
        assert_eq!(strip_preamble("\"Hello'"), "\"Hello'");
        assert_eq!(strip_preamble("'Hello\""), "'Hello\"");
        assert_eq!(strip_preamble("\"Hello"), "\"Hello");
        assert_eq!(strip_preamble("He said \"hello\" to me"), "He said \"hello\" to me");
    }
    #[test] fn fences() {
        assert_eq!(strip_preamble("```\nHello world\n```"), "Hello world");
        assert_eq!(strip_preamble("```\nHello\nworld\n```"), "Hello\nworld");
        assert_eq!(strip_preamble("```Hello```"), "Hello");
        assert_eq!(strip_preamble("```json\nHello world\n```"), "Hello world");
        assert_eq!(strip_preamble("```markdown\n# Title\nBody\n```"), "# Title\nBody");
        assert_eq!(strip_preamble("```\nHello world"), "```\nHello world");
        assert_eq!(strip_preamble("```\n  Hello  \n```"), "Hello");
        assert_eq!(strip_preamble("```\n```"), "");
        assert_eq!(strip_preamble("`Hello`"), "`Hello`");
    }
    #[test] fn combinations() {
        assert_eq!(strip_preamble("Here is the corrected text:\n```\nHello world\n```"), "Hello world");
        assert_eq!(strip_preamble("Sure, here you go:\n```\n\"Hello world\"\n```"), "Hello world");
        assert_eq!(strip_preamble("```\n\"Hello\"\n```"), "Hello");
        assert_eq!(strip_preamble("Here is the corrected text:\n\"Hello world\""), "Hello world");
        assert_eq!(strip_preamble("Here is the corrected text:\nThis is line one.\nThis is line two."), "This is line one.\nThis is line two.");
        assert_eq!(strip_preamble("Here is the corrected text:\n```\n\"\"\n```"), "");
    }
}
