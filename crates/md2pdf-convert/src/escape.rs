//! Arbitrary document text -> text that is safe inside Typst markup.
//!
//! The highest-risk module in the crate. Markdown text is arbitrary; Typst markup is
//! a programming language. An unescaped `#` in a document is arbitrary code execution
//! — `#lorem(50)` in a document body really does run and inject 50 words of filler —
//! and an unbalanced `]` silently terminates the probe harness, corrupting the
//! measurement of an *unrelated* element rather than raising an error.
//!
//! ## The rule, and why it is this broad
//!
//! **Backslash-escape every ASCII punctuation character.** Typst's lexer
//! (`typst-syntax-0.15.1/src/lexer.rs:542-547`) treats `\` followed by any
//! non-whitespace character as an `Escape` token producing that literal character, so
//! the rule is total: it cannot produce invalid markup for any input.
//!
//! It is deliberately wider than the set of characters that are *syntax*, because the
//! evidence showed two distinct failure classes and only a blanket rule catches both:
//!
//! 1. **Syntax errors** — `#`, `$`, `<`, `@`, `]`, `` ` ``, and leading `*` / `_`
//!    failed to compile at all.
//! 2. **Silent text transformations** — Typst rewrites `--` to an en dash, `---` to an
//!    em dash, and `...` to an ellipsis. These are **lexer-level shorthands**
//!    (`lexer.rs:496`), *not* a stylable setting, so no `#set` can turn them off; the
//!    only way to prevent them is to break the character sequence at emission.
//!    Escaping does that for free. `"` and `'` become smart quotes by the same
//!    reasoning, and `\"` bypasses that because the lexer sees an escape, not a quote.
//!
//! GitHub renders none of those substitutions, so suppressing them *is* the fidelity
//! requirement, not a deviation from it.
//!
//! The set was **derived empirically, not recalled** — `tests/roundtrip.rs` compiles
//! candidate text through the real `Typesetter` and compares rendered text with input.
//! 51 of 113 candidates failed before this module existed.
//!
//! Never escape whitespace: `\` followed by a space is a **Linebreak**, not an escape.

/// Escape `text` so it renders literally inside Typst markup.
///
/// Total: defined for all input, including text that is already escaped (which
/// round-trips to a literal backslash followed by the character, as intended).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if needs_escape(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// ASCII punctuation, and nothing else. Whitespace must never be escaped, and
/// non-ASCII characters carry no syntactic meaning in Typst markup.
fn needs_escape(c: char) -> bool {
    c.is_ascii_punctuation()
}

/// Escape `text` for a Typst **string literal** (`"..."`), which is code context and
/// obeys different rules from markup: only `\` and `"` are significant.
///
/// Needed wherever content is passed as a function argument rather than as markup —
/// `#raw("...", lang: "rust")` for code blocks, and image paths.
pub fn escape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(escape("hello world 123"), "hello world 123");
    }

    #[test]
    fn the_injection_case_is_neutralised() {
        // The reason this module exists.
        assert_eq!(escape("#lorem(50)"), r"\#lorem\(50\)");
    }

    #[test]
    fn brackets_that_would_break_the_probe_harness_are_escaped() {
        assert_eq!(escape("a ] b"), r"a \] b");
    }

    #[test]
    fn shorthands_are_broken_up() {
        // Not escapable by any `#set` — must be broken at emission.
        assert_eq!(escape("a -- b"), r"a \-\- b");
        assert_eq!(escape("wait ..."), r"wait \.\.\.");
    }

    #[test]
    fn whitespace_is_never_escaped() {
        // `\` before whitespace is a Linebreak, not an escape.
        assert_eq!(escape("a b\tc\nd"), "a b\tc\nd");
        assert!(!needs_escape(' '));
        assert!(!needs_escape('\n'));
    }

    #[test]
    fn non_ascii_is_left_alone() {
        assert_eq!(escape("café — naïve 日本語"), "café — naïve 日本語");
    }

    #[test]
    fn escaping_is_total_over_ascii() {
        // Every ASCII char must be handled without panicking, and every escape must
        // be exactly two chars or one.
        for c in 0u8..128 {
            let s = (c as char).to_string();
            let e = escape(&s);
            assert!(e.len() <= s.len() + 1, "{c} escaped oddly: {e:?}");
        }
    }

    #[test]
    fn string_literals_escape_only_backslash_and_quote() {
        assert_eq!(escape_string(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(escape_string(r"path\to"), r"path\\to");
        // Markup-significant characters are inert inside a string literal.
        assert_eq!(escape_string("#lorem(50)"), "#lorem(50)");
    }
}
