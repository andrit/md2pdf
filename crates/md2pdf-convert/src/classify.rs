//! Markdown construct -> [`ElementClass`].
//!
//! Load-bearing and easy to get silently wrong. The class carries three jobs
//! (`design/GLOSSARY.md`): it selects the Floor, it selects the overflow predicate,
//! and it defines what "shrink" means. Only `Table` and `Image` are **Atomic**, and
//! only Atomic elements enter the escalation ladder — so classifying a table as prose
//! means it **never gets measured for overflow and silently runs off the page**.
//!
//! That failure is invisible in the output, which is why this module is a plain
//! lookup with an exhaustive test per construct rather than anything clever.

use md2pdf_domain::ElementClass;
use pulldown_cmark::{Event, Tag, TagEnd};

/// Classify one top-level block, given all of its events including `Start`/`End`.
///
/// Takes the whole block rather than just its opening tag because one case needs the
/// contents: a paragraph whose only content is an image is an **Image**, not prose.
/// That distinction matters — an image shrinks by *scale factor* while text shrinks by
/// *font size* ([`ElementClass::shrinks_by_scale`]).
pub fn classify(events: &[Event<'_>]) -> ElementClass {
    match events.first() {
        Some(Event::Start(tag)) => match tag {
            Tag::Heading { .. } => ElementClass::Heading,
            // ponytail: class comes from the outermost construct, so an Atomic table
            // nested inside a Wrappable blockquote or list inherits Quote/List and
            // never enters the escalation ladder — it can overflow silently.
            // ceiling: nested atomics. upgrade: promote nested atomics to their own
            // Elements. See design/plan-conversion-crate.md §2.4.
            Tag::BlockQuote(_) => ElementClass::Quote,
            Tag::CodeBlock(_) => ElementClass::Code,
            Tag::List(_) => ElementClass::List,
            Tag::Table(_) => ElementClass::Table,
            Tag::Paragraph => {
                if is_image_only(events) {
                    ElementClass::Image
                } else {
                    ElementClass::Prose
                }
            }
            // Everything else is text-bearing and Wrappable. `HtmlBlock` lands here
            // deliberately: it is an unsupported construct, and recording *that* is
            // emit's job — classification still has to return something sane.
            _ => ElementClass::Prose,
        },
        // A bare `Rule`, or any stray inline event promoted to its own block.
        _ => ElementClass::Prose,
    }
}

/// True when a paragraph carries one or more images and no other visible content.
///
/// Alt text lives *inside* the image subtree, so it must not count as prose — hence
/// the depth tracking rather than a flat scan.
fn is_image_only(events: &[Event<'_>]) -> bool {
    let mut image_depth = 0usize;
    let mut saw_image = false;
    let mut saw_other = false;

    for ev in events {
        match ev {
            Event::Start(Tag::Image { .. }) => {
                image_depth += 1;
                saw_image = true;
            }
            Event::End(TagEnd::Image) => {
                image_depth = image_depth.saturating_sub(1);
            }
            _ if image_depth > 0 => {}
            Event::Text(t) if t.trim().is_empty() => {}
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {}
            Event::SoftBreak => {}
            _ => saw_other = true,
        }
    }

    saw_image && !saw_other
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    /// Classify the first block of a markdown snippet, through the real parser —
    /// so these assert against events pulldown-cmark actually emits.
    fn class_of(md: &str) -> ElementClass {
        let blocks = parse(md);
        assert!(!blocks.is_empty(), "no blocks parsed from {md:?}");
        blocks[0].class
    }

    #[test]
    fn headings_are_headings() {
        assert_eq!(class_of("# Title"), ElementClass::Heading);
        assert_eq!(class_of("###### Deep"), ElementClass::Heading);
        assert_eq!(class_of("Setext\n======"), ElementClass::Heading);
    }

    #[test]
    fn paragraphs_are_prose() {
        assert_eq!(class_of("just some words"), ElementClass::Prose);
        assert_eq!(
            class_of("words with *emphasis* and `code`"),
            ElementClass::Prose
        );
    }

    #[test]
    fn tables_are_tables_and_therefore_atomic() {
        let c = class_of("| a | b |\n|---|---|\n| 1 | 2 |");
        assert_eq!(c, ElementClass::Table);
        assert!(
            c.is_atomic(),
            "a table that is not Atomic never gets measured"
        );
    }

    #[test]
    fn code_blocks_are_code_and_wrappable() {
        let c = class_of("```rust\nfn main() {}\n```");
        assert_eq!(c, ElementClass::Code);
        // Verified in the spike: `raw` blocks wrap in Typst 0.15.1. Do not "fix".
        assert!(!c.is_atomic());
        assert_eq!(class_of("    indented code"), ElementClass::Code);
    }

    #[test]
    fn lists_are_lists() {
        assert_eq!(class_of("- one\n- two"), ElementClass::List);
        assert_eq!(class_of("1. one\n2. two"), ElementClass::List);
        assert_eq!(class_of("- [ ] task\n- [x] done"), ElementClass::List);
    }

    #[test]
    fn blockquotes_and_gfm_alerts_are_quotes() {
        assert_eq!(class_of("> quoted"), ElementClass::Quote);
        // ENABLE_GFM: alert blockquotes.
        assert_eq!(class_of("> [!WARNING]\n> careful"), ElementClass::Quote);
    }

    #[test]
    fn a_lone_image_is_an_image_so_it_shrinks_by_scale() {
        let c = class_of("![alt text](diagram.png)");
        assert_eq!(c, ElementClass::Image);
        assert!(c.is_atomic());
        assert!(
            c.shrinks_by_scale(),
            "images scale; they do not change font size"
        );
    }

    #[test]
    fn an_image_with_surrounding_text_is_prose() {
        // Text beside the image means the block is not a figure.
        assert_eq!(class_of("see ![alt](d.png) here"), ElementClass::Prose);
    }

    #[test]
    fn alt_text_does_not_make_an_image_into_prose() {
        // The trap: alt text is a Text event *inside* the image subtree.
        assert_eq!(
            class_of("![a long descriptive caption](d.png)"),
            ElementClass::Image
        );
    }

    #[test]
    fn a_rule_classifies_without_panicking() {
        assert_eq!(class_of("---\n"), ElementClass::Prose);
    }

    #[test]
    fn empty_input_is_handled() {
        assert_eq!(classify(&[]), ElementClass::Prose);
    }
}
