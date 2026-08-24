//! Characters no shipped face can draw, and what to draw instead.
//!
//! A character with no glyph renders as **tofu** — an empty box. Nothing in the text
//! layer says so, because extracted text carries the character either way, which is how
//! ✅ and ❌ went unnoticed in roughly a fifth of the corpus until a page was rendered
//! and looked at (T26b, then T28).
//!
//! ## Why the table lives here and not in `md2pdf-typeset`
//!
//! Two different questions, deliberately in two places:
//!
//! | Question | Whose | Where |
//! |---|---|---|
//! | *Can anything draw `U+2705`?* | typographic | `Typesetter::uncovered` |
//! | *What does `✅` **mean**, if we cannot draw it?* | editorial | this table |
//!
//! `convert` has no fonts and must not grow any — it is the crate that stays testable
//! without Typst. So it cannot ask about coverage, and instead holds a small fixed table
//! of substitutions that a person chose.
//!
//! **They cannot drift.** `every_substitution_target_can_actually_be_drawn` in
//! `md2pdf-typeset`'s contract tests asserts that every replacement below is covered by
//! the shipped FontBook, and `every_character_the_corpus_uses_has_a_glyph` asserts that
//! the originals are the ones that need substituting. A font change that broke either
//! turns a test red rather than shipping boxes.

/// Substitutions, chosen deliberately.
///
/// **[measured]** across 146 real documents these are the only two characters with no
/// glyph. Every other symbol the corpus uses — `✓ ✗ ⚠ → ▸`, box drawing, the ballot
/// boxes `convert` emits for task lists — is covered.
///
/// The replacements are the plain-text forms of the same marks: `U+2705` *white heavy
/// check mark* is an emoji-presentation tick, and `U+2713` *check mark* is the same idea
/// a printer can set. Colour is lost, which is why it is recorded as a Compromise rather
/// than done quietly — the author wrote a green tick and gets a black one.
pub const SUBSTITUTIONS: &[(char, char, &str)] = &[
    ('\u{2705}', '\u{2713}', "white heavy check mark"),
    ('\u{274C}', '\u{2717}', "cross mark"),
];

/// Replace what cannot be drawn, reporting each distinct character replaced.
///
/// Returns the rewritten text and the names of the characters that were substituted, so
/// the caller can raise one Compromise per kind rather than one per occurrence — a
/// document with forty ✅ made one decision, not forty.
pub fn substitute(text: &str) -> (std::borrow::Cow<'_, str>, Vec<&'static str>) {
    if !text.chars().any(is_substituted) {
        return (std::borrow::Cow::Borrowed(text), Vec::new());
    }
    let mut names: Vec<&'static str> = Vec::new();
    let out: String = text
        .chars()
        .map(
            |c| match SUBSTITUTIONS.iter().find(|(from, _, _)| *from == c) {
                Some((_, to, name)) => {
                    if !names.contains(name) {
                        names.push(name);
                    }
                    *to
                }
                None => c,
            },
        )
        .collect();
    (std::borrow::Cow::Owned(out), names)
}

fn is_substituted(c: char) -> bool {
    SUBSTITUTIONS.iter().any(|(from, _, _)| *from == c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_without_them_is_untouched_and_not_reallocated() {
        let (out, names) = substitute("an ordinary ✓ line");
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert!(names.is_empty());
    }

    #[test]
    fn the_tick_becomes_one_that_can_be_drawn() {
        let (out, names) = substitute("done ✅");
        assert_eq!(out, "done ✓");
        assert_eq!(names, vec!["white heavy check mark"]);
    }

    #[test]
    fn each_character_is_reported_once_however_often_it_appears() {
        // A document with forty ticks made one decision, not forty. The Diagnostic is
        // read by a person deciding where to look.
        let (out, names) = substitute("✅ a ✅ b ✅ c ❌");
        assert_eq!(out, "✓ a ✓ b ✓ c ✗");
        assert_eq!(names, vec!["white heavy check mark", "cross mark"]);
    }

    #[test]
    fn substitutions_are_not_themselves_substituted() {
        // A replacement that was itself in the table would loop or, worse, silently
        // rewrite twice.
        for (_, to, _) in SUBSTITUTIONS {
            assert!(
                !is_substituted(*to),
                "{to} is both a replacement and a thing to replace"
            );
        }
    }
}
