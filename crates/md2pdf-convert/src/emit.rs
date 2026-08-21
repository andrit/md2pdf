//! Internal block stream -> Typst Markup, one [`Element`] per top-level block.
//!
//! Typst markup syntax is a stable surface, so this crate emits it as plain text and
//! never links the typst crate. See `md2pdf-typeset` for that boundary.
//!
//! ## Function forms, not line-start markers
//!
//! Every construct is emitted as a **function call** — `#heading(level: 2)[...]`,
//! `#list([a], [b])` — rather than as markup shorthand (`== Heading`, `- item`).
//!
//! Shorthand is position-sensitive: it only means anything at the start of a line. An
//! `Element` body is interpolated *both* raw at top level and inside a content block
//! `[...]` (`design/plan-conversion-crate.md` §1.1), and it is also wrapped by
//! `#text(size: ..)[...]` when the ladder shrinks it. Function forms are inert under
//! all of those, so the body invariant holds **by construction** rather than by luck.
//!
//! ## Images are placeholders here
//!
//! Stage 1 never emits `#image(...)`. Typst treats an unresolvable file as a
//! compilation error for the *whole document*, and `World::file()` currently serves
//! nothing — so one image would mean no PDF at all. Every image becomes a visible
//! bordered placeholder plus a `Compromise`. See the plan, §3.

use std::collections::HashMap;
use std::iter::Peekable;
use std::slice::Iter;

use md2pdf_domain::{Compromise, CompromiseKind, Element, ElementClass, Markup};
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};

use crate::escape::{escape, escape_string};
use crate::images::{resolve, ImageRef};
use crate::parse::Block;
use crate::{ImageManifest, SourceContext};

/// Everything one conversion produced: the elements, and the concessions made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Emitted {
    pub elements: Vec<Element>,
    pub compromises: Vec<Compromise>,
    /// Every image that resolved, keyed by the virtual name the markup references.
    pub images: ImageManifest,
}

/// Emit every block, in source order.
///
/// Footnote definitions produce no `Element` — they are absorbed into the element that
/// references them (§2.4). Everything else becomes exactly one.
pub fn emit(blocks: &[Block<'_>], ctx: &SourceContext) -> Emitted {
    let footnotes = collect_footnotes(blocks, ctx);
    let mut out = Emitted::default();
    let mut order = 0u32;

    for block in blocks {
        if is_footnote_definition(block) {
            continue;
        }
        let mut em = Emitter {
            ctx,
            table: None,
            footnotes: &footnotes,
            pending: Vec::new(),
            images: ImageManifest::new(),
            in_header: false,
        };
        let body = em.sequence(&mut block.events.iter().peekable(), None);
        let mut body = body.trim_end().to_string();

        // Invariant: a block that made a concession always produces an Element.
        //
        // Without this, an unsupported construct that renders to nothing (raw HTML)
        // would drop out here and take its Compromise with it — the content would
        // vanish AND the record of it vanishing would too. A Compromise needs an
        // ElementId to be addressable at the attention gate, so it needs an Element.
        if body.is_empty() {
            if em.pending.is_empty() {
                continue;
            }
            body = placeholder("unsupported content");
        }

        // `order` is assigned from a single monotonic counter, so it is unique across
        // the document. `harvest.rs:27` resolves probe metadata back to elements by
        // `order` alone, and a duplicate would silently bind a decision to the wrong
        // element. See the plan, §1.2.
        // A table gets an alternate rendering with fractional columns, which fills the
        // width available and wraps inside cells rather than sizing columns to content.
        //
        // Content-sized columns read better when they fit, and cannot reflow when they
        // do not: the ladder's only lever is shrinking the whole table, and past the
        // floor that meant clipping — losing the right-hand columns outright. Carrying
        // both forms lets the probe choose, so a table never has to lose content.
        //
        // Only whole-table blocks qualify. A table nested inside a blockquote is emitted
        // into that block's body and is not separable here — the same nested-atomics
        // ceiling recorded in `classify.rs`.
        let element = match em.table.take() {
            Some((columns, cells)) if block.class == ElementClass::Table => {
                let fr = vec!["1fr"; columns].join(", ");
                Element::with_reflow(
                    order,
                    block.class,
                    Markup::raw(body),
                    Markup::raw(format!("#table(columns: ({fr}), {cells})")),
                )
            }
            _ => Element::new(order, block.class, Markup::raw(body)),
        };
        for kind in em.pending {
            out.compromises.push(Compromise {
                id: element.id,
                kind,
                page: None,
            });
        }
        out.images.extend(em.images);
        out.elements.push(element);
        order += 1;
    }

    out
}

fn is_footnote_definition(block: &Block<'_>) -> bool {
    matches!(
        block.events.first(),
        Some(Event::Start(Tag::FootnoteDefinition(_)))
    )
}

/// Pre-render every footnote definition, keyed by label.
///
/// GFM puts definitions at the bottom as separate blocks; Typst wants `#footnote[..]`
/// inline at the reference. So definitions are rendered once, up front, and spliced in
/// wherever they are referenced.
fn collect_footnotes(blocks: &[Block<'_>], ctx: &SourceContext) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for block in blocks {
        let Some(Event::Start(Tag::FootnoteDefinition(label))) = block.events.first() else {
            continue;
        };
        // Definitions may reference other footnotes; those resolve to literal text
        // rather than recursing, which is GFM's own behaviour for an unresolved ref.
        let mut em = Emitter {
            ctx,
            table: None,
            footnotes: &HashMap::new(),
            pending: Vec::new(),
            images: ImageManifest::new(),
            in_header: false,
        };
        let inner = em.sequence(&mut block.events.iter().peekable(), None);
        map.insert(label.to_string(), inner.trim().to_string());
    }
    map
}

struct Emitter<'m> {
    ctx: &'m SourceContext<'m>,
    /// Columns and cells of the table in this block, if it is one — so the block can
    /// also be expressed in its always-fitting form.
    table: Option<(usize, String)>,
    footnotes: &'m HashMap<String, String>,
    /// Compromises for the block being emitted; the Element id is attached once the
    /// body is known.
    pending: Vec<CompromiseKind>,
    images: ImageManifest,
    /// Table header cells render bold, as GitHub does.
    in_header: bool,
}

impl Emitter<'_> {
    /// Render events until `until` closes the current construct, or the stream ends.
    fn sequence(
        &mut self,
        events: &mut Peekable<Iter<'_, Event<'_>>>,
        until: Option<TagEnd>,
    ) -> String {
        let mut out = String::new();
        while let Some(event) = events.next() {
            match event {
                Event::End(end) => {
                    if until.as_ref() == Some(end) {
                        return out;
                    }
                    // An End we did not open: the caller's frame will handle it.
                }
                Event::Start(tag) => out.push_str(&self.element(tag, events)),
                Event::Text(t) => out.push_str(&escape(t)),
                Event::Code(t) => {
                    out.push_str(&format!("#raw(\"{}\")", escape_string(t)));
                }
                Event::SoftBreak => out.push(' '),
                Event::HardBreak => out.push_str("#linebreak()"),
                Event::Rule => out.push_str("#line(length: 100%)"),
                Event::TaskListMarker(done) => {
                    out.push_str(if *done { "☑ " } else { "☐ " });
                }
                Event::FootnoteReference(label) => {
                    out.push_str(&self.footnote(label));
                }
                // MATH is not enabled, so these cannot appear; HTML can.
                //
                // Comments are dropped silently — GitHub does not render them either,
                // so passing one through is not a concession worth reporting.
                Event::Html(h) => {
                    if !is_html_comment(h) {
                        self.unsupported(format!("html: {}", h.trim()));
                        out.push_str(&placeholder("unsupported html"));
                    }
                }
                Event::InlineHtml(h) => {
                    if !is_html_comment(h) {
                        self.unsupported(format!("inline html: {}", h.trim()));
                    }
                }
                Event::InlineMath(_) | Event::DisplayMath(_) => {
                    self.unsupported("math".to_string());
                }
            }
        }
        out
    }

    /// Render one `Start(tag)` and everything up to its matching `End`.
    fn element(&mut self, tag: &Tag<'_>, events: &mut Peekable<Iter<'_, Event<'_>>>) -> String {
        let end = tag.to_end();
        match tag {
            Tag::Paragraph => {
                let inner = self.sequence(events, Some(end));
                format!("{}\n\n", inner.trim())
            }
            Tag::Heading { level, .. } => {
                let inner = self.sequence(events, Some(end));
                format!(
                    "#heading(level: {})[{}]\n\n",
                    level_of(*level),
                    inner.trim()
                )
            }
            Tag::BlockQuote(kind) => {
                let inner = self.sequence(events, Some(end));
                self.blockquote(*kind, inner.trim())
            }
            Tag::CodeBlock(kind) => {
                // Code text must NOT go through markup escaping: it is passed as a
                // string argument to `#raw`, which obeys string-literal rules instead.
                // Running both produced `fn main\\(\\)` — escaped twice.
                let mut code = String::new();
                for event in events.by_ref() {
                    match event {
                        Event::End(e) if *e == end => break,
                        Event::Text(t) => code.push_str(t),
                        _ => {}
                    }
                }
                code_block(kind, &code)
            }
            Tag::List(first) => {
                let items = self.items(events, end);
                let joined = items.join(", ");
                match first {
                    Some(start) => format!("#enum(start: {start}, {joined})\n\n"),
                    None => format!("#list({joined})\n\n"),
                }
            }
            Tag::Item => format!("[{}]", self.sequence(events, Some(end)).trim()),
            Tag::Table(alignments) => {
                let columns = alignments.len().max(1);
                let cells = self.sequence(events, Some(end));
                // Remember the pieces so `emit` can also build the reflow alternate.
                self.table = Some((columns, cells.clone()));
                format!("#table(columns: {columns}, {cells})\n\n")
            }
            // Header cells are bold, matching how GitHub renders them.
            Tag::TableHead => {
                self.in_header = true;
                let inner = self.sequence(events, Some(end));
                self.in_header = false;
                inner
            }
            Tag::TableRow => self.sequence(events, Some(end)),
            Tag::TableCell => {
                let inner = self.sequence(events, Some(end));
                let inner = inner.trim();
                if self.in_header {
                    format!("[#strong[{inner}]], ")
                } else {
                    format!("[{inner}], ")
                }
            }
            Tag::Emphasis => format!("#emph[{}]", self.sequence(events, Some(end))),
            Tag::Strong => format!("#strong[{}]", self.sequence(events, Some(end))),
            Tag::Strikethrough => format!("#strike[{}]", self.sequence(events, Some(end))),
            Tag::Link { dest_url, .. } => {
                let inner = self.sequence(events, Some(end));
                format!("#link(\"{}\")[{}]", escape_string(dest_url), inner)
            }
            Tag::Image { dest_url, .. } => {
                let alt = self.sequence(events, Some(end));
                self.image(dest_url, alt.trim())
            }
            Tag::FootnoteDefinition(_) => self.sequence(events, Some(end)),
            // Enabled-but-unmodelled, and anything a future option turns on.
            other => {
                self.unsupported(format!("{other:?}"));
                self.sequence(events, Some(end))
            }
        }
    }

    fn items(&mut self, events: &mut Peekable<Iter<'_, Event<'_>>>, end: TagEnd) -> Vec<String> {
        let mut items = Vec::new();
        while let Some(event) = events.peek() {
            match event {
                Event::End(e) if *e == end => {
                    events.next();
                    break;
                }
                Event::Start(Tag::Item) => {
                    events.next();
                    items.push(format!(
                        "[{}]",
                        self.sequence(events, Some(TagEnd::Item)).trim()
                    ));
                }
                _ => {
                    events.next();
                }
            }
        }
        items
    }

    fn blockquote(&mut self, kind: Option<BlockQuoteKind>, inner: &str) -> String {
        match kind {
            // GFM alerts carry a label GitHub renders prominently; keep the word.
            Some(k) => format!(
                "#quote(block: true)[#strong[{}] #linebreak() {inner}]\n\n",
                alert_label(k)
            ),
            None => format!("#quote(block: true)[{inner}]\n\n"),
        }
    }

    fn footnote(&mut self, label: &str) -> String {
        match self.footnotes.get(label) {
            Some(body) => format!("#footnote[{body}]"),
            // GFM renders an undefined reference as literal text.
            None => escape(&format!("[^{label}]")),
        }
    }

    fn unsupported(&mut self, construct: String) {
        self.pending
            .push(CompromiseKind::UnsupportedConstruct { construct });
    }

    /// Resolve an image, or degrade visibly and say why.
    ///
    /// `#image(..)` is emitted **only** for a path that resolved and was recorded in
    /// the manifest. Emitting one Typst cannot satisfy fails the *whole document*, not
    /// one element — which is the failure this arm exists to prevent.
    ///
    /// Alt text is dropped once an image resolves: it is a fallback, a PDF has no
    /// alt-text channel, and GitHub shows the picture rather than the words. It stays
    /// in the placeholder, where it is the only thing left to show.
    fn image(&mut self, dest: &str, alt: &str) -> String {
        match resolve(dest, self.ctx.source_dir, self.ctx.images) {
            ImageRef::Resolved {
                virtual_name,
                absolute,
            } => {
                // Wrapped in a `box` so the image is **inline-level**. A bare
                // `#image(..)` is block-level in Typst and forces a line break, which
                // splits "text ![x](y.png) more" across three lines — GitHub keeps it
                // on one. A lone image in its own paragraph renders identically either
                // way, so the box is always correct and never costs anything.
                let markup = format!("#box(image(\"{}\"))", escape_string(&virtual_name));
                self.images.insert(virtual_name, absolute);
                markup
            }
            ImageRef::Missing { shown } => {
                self.pending.push(CompromiseKind::ImageMissing);
                image_placeholder("image not found", &shown, alt)
            }
            ImageRef::Remote { shown } => {
                self.pending.push(CompromiseKind::ImageSkipped);
                image_placeholder("remote image skipped", &shown, alt)
            }
            ImageRef::Unsupported { shown, why } => {
                self.unsupported(format!("image ({why}): {shown}"));
                image_placeholder(why, &shown, alt)
            }
        }
    }
}

/// A visible marker for an image that could not be embedded, naming both the reason
/// and the file so the page and the diagnostic agree.
fn image_placeholder(why: &str, shown: &str, alt: &str) -> String {
    let caption = if alt.is_empty() {
        escape(shown)
    } else {
        escape(alt)
    };
    format!(
        "#box(stroke: 0.5pt, inset: 6pt)[#emph[{}:] {caption}]",
        escape(why)
    )
}

/// A visible marker for content md2pdf could not render. Used for images (Stage 1)
/// and for unsupported constructs — the user must be able to *see* the gap, not just
/// read about it in the diagnostic.
fn placeholder(what: &str) -> String {
    format!("#box(stroke: 0.5pt, inset: 6pt)[#emph[{}]]", escape(what))
}

/// GitHub does not render HTML comments either, so dropping one is not a concession.
fn is_html_comment(html: &str) -> bool {
    html.trim_start().starts_with("<!--")
}

fn level_of(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn alert_label(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "Note",
        BlockQuoteKind::Tip => "Tip",
        BlockQuoteKind::Important => "Important",
        BlockQuoteKind::Warning => "Warning",
        BlockQuoteKind::Caution => "Caution",
    }
}

/// Code is passed as a **string argument**, not markup, so it obeys string-literal
/// escaping rather than markup escaping. Typst highlights `raw` natively when given a
/// language, which is why no syntax highlighter is needed.
fn code_block(kind: &CodeBlockKind<'_>, code: &str) -> String {
    let code = escape_string(code.trim_end_matches('\n'));
    match kind {
        CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
            let lang = lang.split_whitespace().next().unwrap_or("").to_lowercase();
            format!(
                "#raw(\"{code}\", lang: \"{}\", block: true)\n\n",
                escape_string(&lang)
            )
        }
        _ => format!("#raw(\"{code}\", block: true)\n\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::SourceContext;
    use md2pdf_domain::ElementClass;

    fn run(md: &str) -> Emitted {
        emit(&parse(md), &SourceContext::none())
    }

    fn bodies(md: &str) -> Vec<String> {
        run(md)
            .elements
            .iter()
            .map(|e| e.body.as_str().to_string())
            .collect()
    }

    #[test]
    fn a_heading_uses_the_function_form() {
        assert_eq!(bodies("## Title"), vec!["#heading(level: 2)[Title]"]);
    }

    #[test]
    fn text_is_escaped_on_the_way_through() {
        // The T5 guarantee must survive emission.
        let b = &bodies("costs #5 and [brackets]")[0];
        assert!(b.contains(r"\#5"), "hash not escaped: {b}");
        assert!(b.contains(r"\[brackets\]"), "brackets not escaped: {b}");
    }

    #[test]
    fn lists_become_list_and_enum_calls() {
        assert_eq!(bodies("- a\n- b"), vec!["#list([a], [b])"]);
        assert_eq!(bodies("3. a\n4. b"), vec!["#enum(start: 3, [a], [b])"]);
    }

    #[test]
    fn task_markers_render_as_checkboxes() {
        let b = &bodies("- [x] done\n- [ ] todo")[0];
        assert!(b.contains('☑'), "no checked box: {b}");
        assert!(b.contains('☐'), "no empty box: {b}");
    }

    #[test]
    fn tables_carry_their_column_count_and_bold_headers() {
        let b = &bodies("| a | b |\n|---|---|\n| 1 | 2 |")[0];
        assert!(b.starts_with("#table(columns: 2,"), "wrong columns: {b}");
        assert!(b.contains("[#strong[a]]"), "header not bold: {b}");
        assert!(b.contains("[1]"), "body cell missing: {b}");
    }

    #[test]
    fn code_blocks_pass_the_language_through_for_typst_to_highlight() {
        let b = &bodies("```rust\nfn main() {}\n```")[0];
        assert!(b.contains("lang: \"rust\""), "language lost: {b}");
        assert!(b.contains("block: true"), "not a block: {b}");
        // Code is a string argument, so markup escaping must NOT apply.
        assert!(b.contains("fn main() {}"), "code was markup-escaped: {b}");
    }

    #[test]
    fn inline_code_uses_raw_with_string_escaping() {
        let b = &bodies("use `a #b` here")[0];
        assert!(b.contains(r##"#raw("a #b")"##), "inline code wrong: {b}");
    }

    #[test]
    fn gfm_alerts_keep_their_label() {
        let b = &bodies("> [!WARNING]\n> careful")[0];
        assert!(b.contains("Warning"), "alert label lost: {b}");
    }

    #[test]
    fn an_unresolved_image_never_emits_an_image_call() {
        // The failure guard. A file reference Typst cannot satisfy fails the WHOLE
        // document — not one element — so an unresolved image must never produce
        // `#image`. Stated as the invariant rather than as Stage-1 behaviour, because
        // the invariant is what survives images becoming resolvable.
        let out = run("![a diagram](d.png)");
        let b = out.elements[0].body.as_str();
        assert!(
            !b.contains("#image("),
            "unresolved image emitted #image: {b}"
        );
        assert!(
            b.contains("a diagram"),
            "alt text lost from placeholder: {b}"
        );
        assert!(
            out.images.is_empty(),
            "nothing resolved, so nothing to register"
        );
        assert!(
            out.compromises
                .iter()
                .any(|c| c.kind == CompromiseKind::ImageMissing),
            "no ImageMissing recorded: {:?}",
            out.compromises
        );
    }

    #[test]
    fn every_emitted_image_call_is_backed_by_the_manifest() {
        // The strong form of the guard, as a property: whatever the document, every
        // `#image("name")` md2pdf emits must be a name the engine will register.
        for md in [
            "![a](x.png)",
            "text ![b](y.jpg) more",
            "![c](https://x.test/z.png)",
            "![d](data:image/png;base64,AAA)",
            "no images at all",
        ] {
            let out = run(md);
            for el in &out.elements {
                for name in image_names(el.body.as_str()) {
                    assert!(
                        out.images.contains_key(&name),
                        "{md:?}: emitted #image({name:?}) with no manifest entry"
                    );
                }
            }
        }
    }

    /// Every `#image("…")` name in a body.
    fn image_names(body: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = body;
        while let Some(i) = rest.find("#image(\"") {
            rest = &rest[i + 8..];
            if let Some(end) = rest.find('"') {
                names.push(rest[..end].to_string());
                rest = &rest[end..];
            }
        }
        names
    }

    #[test]
    fn a_remote_image_is_skipped_and_recorded() {
        let out = run("![alt](https://x.test/a.png)");
        assert!(out
            .compromises
            .iter()
            .any(|c| c.kind == CompromiseKind::ImageSkipped));
        assert!(out.images.is_empty());
    }

    #[test]
    fn footnote_definitions_are_absorbed_not_emitted() {
        let out = run("Claim[^1].\n\n[^1]: The evidence.\n");
        assert_eq!(
            out.elements.len(),
            1,
            "definition leaked as its own element: {:?}",
            out.elements.iter().map(|e| e.class).collect::<Vec<_>>()
        );
        let b = out.elements[0].body.as_str();
        assert!(b.contains("#footnote["), "not inlined: {b}");
        assert!(b.contains("The evidence"), "definition body lost: {b}");
    }

    #[test]
    fn an_undefined_footnote_reference_becomes_literal_text() {
        let b = &bodies("Claim[^missing].")[0];
        assert!(!b.contains("#footnote["), "invented a footnote: {b}");
        assert!(b.contains(r"\[\^missing\]"), "not literal: {b}");
    }

    #[test]
    fn html_is_recorded_as_unsupported() {
        let out = run("<div>raw html</div>\n");
        assert!(
            out.compromises
                .iter()
                .any(|c| matches!(&c.kind, CompromiseKind::UnsupportedConstruct { construct } if construct.starts_with("html"))),
            "html not flagged: {:?}",
            out.compromises
        );
    }

    #[test]
    fn orders_are_unique_and_ascending() {
        // harvest.rs:27 resolves decisions by `order` alone — a duplicate binds a
        // decision to the wrong element, silently.
        let md = "# A\n\npara\n\n- x\n\n> q\n\n```\nc\n```\n\n| h |\n|---|\n| v |\n";
        let orders: Vec<u32> = run(md).elements.iter().map(|e| e.id.order).collect();
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(orders.len(), sorted.len(), "duplicate orders: {orders:?}");
        assert!(
            orders.windows(2).all(|w| w[0] < w[1]),
            "not ascending: {orders:?}"
        );
    }

    #[test]
    fn no_body_carries_trailing_whitespace() {
        // render.rs appends `#v(0.65em)`; a body ending in its own space doubles it.
        let md = "# A\n\npara\n\n- x\n\n> q\n";
        for body in bodies(md) {
            assert_eq!(body, body.trim_end(), "trailing space in {body:?}");
        }
    }

    #[test]
    fn classes_survive_emission() {
        let out = run("# H\n\npara\n\n| a |\n|---|\n| 1 |\n");
        let classes: Vec<ElementClass> = out.elements.iter().map(|e| e.class).collect();
        assert_eq!(
            classes,
            vec![
                ElementClass::Heading,
                ElementClass::Prose,
                ElementClass::Table
            ]
        );
    }

    #[test]
    fn empty_input_emits_nothing() {
        assert!(run("").elements.is_empty());
    }
}
