//! `pulldown-cmark` events -> the internal block stream.
//!
//! `pulldown-cmark` emits a flat, balanced stream of `Start`/`End` events. md2pdf needs
//! **top-level blocks**, because one top-level block becomes one `Element` and an
//! `Element` is the unit the escalation ladder measures and decides on.
//!
//! So this module folds the stream by nesting depth: a code block inside a list stays
//! *inside* that list's block rather than becoming a sibling. Class comes from the
//! outermost construct.

use md2pdf_domain::ElementClass;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::classify::classify;

/// One top-level block: everything that will become a single `Element`.
#[derive(Debug, Clone, PartialEq)]
pub struct Block<'a> {
    pub class: ElementClass,
    /// Every event in the block, including its own `Start` and `End`. Retaining the
    /// events rather than re-modelling them into a bespoke AST keeps this DRY —
    /// `emit` walks these directly.
    pub events: Vec<Event<'a>>,
}

/// The parser options, and the reasoning for each.
///
/// Every non-CommonMark feature is opt-in; nothing here is on by default. Verified
/// against the vendored source (`pulldown-cmark-0.12.2/src/lib.rs:520-600`) rather
/// than assumed — this settles open item 5 from `design/event-storm.md`.
///
/// **Deliberately off:** `SMART_PUNCTUATION` rewrites the user's text and would break
/// the escaping round-trip property; that is a typographic choice belonging to the
/// Template. `HEADING_ATTRIBUTES` is HTML-targeted and has no PDF meaning. `MATH`
/// emits TeX, and TeX->Typst translation is its own project. `OLD_FOOTNOTES` is
/// superseded, `DEFINITION_LIST` is not GFM, and `PLUSES_DELIMITED_METADATA_BLOCKS`
/// is TOML front matter, which is far rarer than YAML.
pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        // GitHub alert blockquotes: [!NOTE], [!WARNING], ...
        | Options::ENABLE_GFM
        // Not to *render* front matter, but to recognise it so it can be skipped.
        // With this off, `---\ntitle: x\n---` parses as a thematic break plus a
        // paragraph — i.e. visible garbage at the top of every such PDF.
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

/// Parse markdown into top-level blocks, in source order.
///
/// Front matter is recognised and **dropped**: it is document metadata, not content.
pub fn parse(markdown: &str) -> Vec<Block<'_>> {
    let mut blocks = Vec::new();
    let mut current: Vec<Event<'_>> = Vec::new();
    let mut depth = 0usize;
    let mut in_metadata = false;

    for event in Parser::new_ext(markdown, options()) {
        // Front matter: swallow the whole block, contents included.
        match &event {
            Event::Start(Tag::MetadataBlock(_)) => {
                in_metadata = true;
                continue;
            }
            Event::End(TagEnd::MetadataBlock(_)) => {
                in_metadata = false;
                continue;
            }
            _ if in_metadata => continue,
            _ => {}
        }

        match &event {
            Event::Start(_) => {
                depth += 1;
                current.push(event);
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                current.push(event);
                if depth == 0 {
                    blocks.push(finish(std::mem::take(&mut current)));
                }
            }
            // A standalone event at top level — `Rule` is the realistic case — is its
            // own block. Nested, it just joins the block being accumulated.
            _ => {
                current.push(event);
                if depth == 0 {
                    blocks.push(finish(std::mem::take(&mut current)));
                }
            }
        }
    }

    // Defensive: an unbalanced stream would otherwise silently drop trailing content.
    // pulldown-cmark guarantees balance, so this should stay unreachable.
    if !current.is_empty() {
        blocks.push(finish(current));
    }

    blocks
}

fn finish(events: Vec<Event<'_>>) -> Block<'_> {
    Block {
        class: classify(&events),
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes(md: &str) -> Vec<ElementClass> {
        parse(md).iter().map(|b| b.class).collect()
    }

    #[test]
    fn top_level_blocks_are_separated_in_source_order() {
        assert_eq!(
            classes("# Title\n\nSome prose.\n\n- a\n- b\n"),
            vec![
                ElementClass::Heading,
                ElementClass::Prose,
                ElementClass::List
            ]
        );
    }

    #[test]
    fn a_code_block_inside_a_list_stays_inside_the_list() {
        // The nesting case the fold exists for. Two blocks would mean the code block
        // became a sibling and the list lost its body.
        let md = "- item\n\n  ```rust\n  fn main() {}\n  ```\n";
        let blocks = parse(md);
        assert_eq!(
            blocks.len(),
            1,
            "expected one List block, got {:?}",
            classes(md)
        );
        assert_eq!(blocks[0].class, ElementClass::List);
        assert!(
            blocks[0]
                .events
                .iter()
                .any(|e| matches!(e, Event::Start(Tag::CodeBlock(_)))),
            "the code block should be inside the list's events"
        );
    }

    #[test]
    fn a_table_inside_a_blockquote_is_one_quote_block() {
        // ponytail: this is the known ceiling — the table is Atomic but the block is
        // classified Quote (Wrappable), so it never enters the escalation ladder and
        // can overflow. ceiling: nested atomics. upgrade: promote nested atomics to
        // their own Elements. See design/plan-conversion-crate.md §2.4.
        let blocks = parse("> | a | b |\n> |---|---|\n> | 1 | 2 |\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].class, ElementClass::Quote);
        assert!(!blocks[0].class.is_atomic());
    }

    #[test]
    fn nested_lists_stay_one_block() {
        let blocks = parse("- outer\n  - inner\n    - deeper\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].class, ElementClass::List);
    }

    #[test]
    fn yaml_front_matter_is_dropped_entirely() {
        let md = "---\ntitle: My Doc\nauthor: Someone\n---\n\nReal content.\n";
        let blocks = parse(md);
        assert_eq!(
            blocks.len(),
            1,
            "front matter leaked: {:?}",
            blocks.iter().map(|b| b.class).collect::<Vec<_>>()
        );
        assert_eq!(blocks[0].class, ElementClass::Prose);
        // And none of its text survived.
        let text: String = blocks[0]
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) => Some(t.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            !text.contains("title"),
            "front matter text leaked: {text:?}"
        );
    }

    #[test]
    fn a_document_without_front_matter_keeps_its_thematic_break() {
        // The flip side: `---` as a real horizontal rule must survive.
        let blocks = parse("Some prose.\n\n---\n\nMore prose.\n");
        assert_eq!(blocks.len(), 3);
        assert!(blocks.iter().any(|b| b.events.contains(&Event::Rule)));
    }

    #[test]
    fn gfm_extensions_are_actually_enabled() {
        // Each of these parses as plain prose if its option is missing.
        assert_eq!(classes("| a |\n|---|\n| 1 |")[0], ElementClass::Table);
        assert!(parse("~~struck~~")[0]
            .events
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::Strikethrough))));
        assert!(parse("- [x] done")[0]
            .events
            .iter()
            .any(|e| matches!(e, Event::TaskListMarker(true))));
        assert!(parse("text[^1]\n\n[^1]: note").iter().any(|b| b
            .events
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::FootnoteDefinition(_))))));
    }

    #[test]
    fn smart_punctuation_is_off_so_text_reaches_escaping_intact() {
        // If SMART_PUNCTUATION were on, the parser would replace these before md2pdf
        // ever saw them, and the escaping round-trip could not hold the line.
        let blocks = parse(r#"a -- b ... c "quoted""#);
        let text: String = blocks[0]
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) => Some(t.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            text.contains("--"),
            "en dash substituted by the parser: {text:?}"
        );
        assert!(
            text.contains("..."),
            "ellipsis substituted by the parser: {text:?}"
        );
        assert!(
            text.contains('"'),
            "smart quotes substituted by the parser: {text:?}"
        );
    }

    #[test]
    fn empty_and_whitespace_input_produce_no_blocks() {
        assert!(parse("").is_empty());
        assert!(parse("   \n\n  \n").is_empty());
    }

    #[test]
    fn every_block_is_balanced() {
        let md = "# T\n\n> quote\n\n- a\n  - b\n\n| x |\n|---|\n| 1 |\n\n```\ncode\n```\n";
        for block in parse(md) {
            let starts = block
                .events
                .iter()
                .filter(|e| matches!(e, Event::Start(_)))
                .count();
            let ends = block
                .events
                .iter()
                .filter(|e| matches!(e, Event::End(_)))
                .count();
            assert_eq!(starts, ends, "unbalanced block: {:?}", block.class);
        }
    }
}
