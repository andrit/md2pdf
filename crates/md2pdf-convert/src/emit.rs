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
            cell_widths: Vec::new(),
            cell_index: 0,
            footnotes: &footnotes,
            pending: Vec::new(),
            images: ImageManifest::new(),
            in_header: false,
            breakable: None,
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
        // A table gets an alternate rendering whose wide columns share the leftover
        // width, so it fills the space available and wraps inside the cells that hold
        // prose rather than sizing every column to its content.
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
            Some((columns, _)) if block.class == ElementClass::Table => {
                let spec = column_spec(columns, &em.cell_widths);
                // The alternate's cells are emitted a *second* time, with breaking on.
                // Re-running the same code path rather than rewriting the first pass's
                // markup: both forms then cannot drift, and nothing has to parse Typst
                // to find where a break may safely go.
                let cells = breakable_cells(block, ctx, &footnotes, columns, &em.cell_widths)
                    .unwrap_or_default();
                Element::with_reflow(
                    order,
                    block.class,
                    Markup::raw(body),
                    Markup::raw(format!("#table(columns: ({spec}), {cells})")),
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

/// Emit one table block's cells again, with break opportunities in long runs.
///
/// The second pass's `pending` compromises and `images` are **discarded**: the first
/// pass already recorded them against this element, and counting them twice would
/// inflate every Diagnostic — the attention gate would report a missing image twice for
/// one table.
fn breakable_cells(
    block: &Block<'_>,
    ctx: &SourceContext,
    footnotes: &HashMap<String, String>,
    columns: usize,
    widths: &[usize],
) -> Option<String> {
    let mut em = Emitter {
        ctx,
        table: None,
        cell_widths: Vec::new(),
        cell_index: 0,
        footnotes,
        pending: Vec::new(),
        images: ImageManifest::new(),
        in_header: false,
        breakable: Some(break_limits(widths, columns)),
    };
    em.sequence(&mut block.events.iter().peekable(), None);
    em.table.take().map(|(_, cells)| cells)
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
            cell_widths: Vec::new(),
            cell_index: 0,
            footnotes: &HashMap::new(),
            pending: Vec::new(),
            images: ImageManifest::new(),
            in_header: false,
            breakable: None,
        };
        let inner = em.sequence(&mut block.events.iter().peekable(), None);
        map.insert(label.to_string(), inner.trim().to_string());
    }
    map
}

/// Roughly how many characters span the full text width at the base size.
///
/// ponytail: `convert` has no `Template`, so the page width cannot be known here — 96 is
/// A4 minus margins at 10pt, measured. ceiling: a much narrower or wider template breaks
/// the estimate. upgrade: pass the Template into conversion, or move break insertion
/// into the typeset layer where the width is known.
const CHARS_ACROSS: usize = 96;

/// The longest run left alone **in each column**, from the same weights that size them.
///
/// A single threshold per table was the previous rule, and it was wrong for the same
/// reason equal `1fr` columns were: it assumes every column has the same room. Once the
/// spec is deliberately lopsided — `(1fr, 1fr, 1fr, 6fr)` — a weight-1 column holds a
/// ninth of the width, and a threshold computed as `96 / 4` lets a 20-character run sit
/// in a column with space for eleven. Measured: that is what still ran off the page
/// after T29b (**F8**).
///
/// So each column gets the threshold its own share implies. `fr` bounds the *table*;
/// this is what bounds the *cell*.
fn break_limits(widths: &[usize], columns: usize) -> Vec<usize> {
    let weights = column_weights(widths, columns);
    let total: usize = weights.iter().sum::<usize>().max(1);
    weights
        .iter()
        .map(|w| (CHARS_ACROSS * w / total).clamp(6, 48))
        .collect()
}

/// The weight each column carries, shared by the column spec and the break limits so the
/// two cannot disagree about how wide a column is meant to be.
fn column_weights(widths: &[usize], columns: usize) -> Vec<usize> {
    let widest = widths.iter().copied().max().unwrap_or(0).max(1);
    (0..columns)
        .map(|i| (widths.get(i).copied().unwrap_or(0) * WIDEST_WEIGHT / widest).max(1))
        .collect()
}

/// Zero-width space — invisible, and a place Typst may wrap.
const BREAK: char = '\u{200b}';

/// Give long unbroken runs somewhere to wrap.
///
/// Typst cannot break a run of characters that offers no opportunity, so a cell holding
/// `completeSubmission(existingId)` is at least as wide as that string however the
/// columns are specified — the table overflows the page and the cells overprint each
/// other, while the ladder records `Reflowed` and reads as handled.
///
/// **[measured]** a zero-width space works in plain text *and* inside `#raw`, which is
/// the load-bearing fact: inline code is 77% of the long runs in the corpus, and raw
/// text is otherwise rendered verbatim. See `design/plan-t29.md`.
///
/// Applied **before** escaping, so a break can never land inside an escape sequence.
/// `U+200B` is not ASCII punctuation, so `escape` passes it through untouched.
fn offer_breaks(text: &str, limit: usize) -> String {
    let every = (limit / 2).max(4);
    let mut out = String::with_capacity(text.len());
    let mut run = 0usize;
    // Measured over the whole run first: a short word must not be split just because it
    // follows a long one.
    for word in text.split_inclusive(char::is_whitespace) {
        let trimmed = word.trim_end();
        if trimmed.chars().count() <= limit {
            out.push_str(word);
            continue;
        }
        for c in word.chars() {
            if c.is_whitespace() {
                run = 0;
            } else {
                if run > 0 && run.is_multiple_of(every) {
                    out.push(BREAK);
                }
                run += 1;
            }
            out.push(c);
        }
    }
    out
}

/// A column shares the leftover width when it is at least this fraction as wide as the
/// widest column in the same table. Relative rather than absolute, because the corpus
/// offers no natural cut: per-column max cell length runs p10 = 8, p50 = 29, p90 = 84
/// characters across 1215 columns, a smooth spread. See `design/plan-reflow-columns.md`.
/// The weight given to the widest column. Narrow ones get a proportional share of this,
/// never less than 1.
///
/// Small on purpose: the spec is meant to be readable in a diff and in the census, and
/// finer gradations than sixths make no visible difference at A4.
const WIDEST_WEIGHT: usize = 6;

/// Build the reflow alternate's column spec: **weighted fractional columns**, in
/// proportion to how much text each column holds.
///
/// Two failed designs preceded this, and both failures are the reason for the shape:
///
/// 1. **Equal `1fr` everywhere** gave a column holding "P1" the same width as one
///    holding a paragraph. Measured against real tables, deep shrinking read better than
///    that — which is what stalled the whole reordering question (T26a2).
/// 2. **`auto` for narrow columns, `1fr` for wide ones** fixed the proportions and broke
///    the fitting guarantee: `auto` in Typst means *size to content and do not shrink*,
///    so several near-threshold columns sum past the page and the single `1fr` is left
///    with negative space. Four real tables ran off the page that way (F8).
///
/// Fractional columns **divide** the width available, so the total can never exceed it —
/// that is the guarantee `Reflow` needs to sit where it does, one rung before `Clip`.
/// Weighting them keeps the proportionality that `auto` was there to provide.
fn column_spec(columns: usize, widths: &[usize]) -> String {
    column_weights(widths, columns)
        .iter()
        .map(|w| format!("{w}fr"))
        .collect::<Vec<_>>()
        .join(", ")
}

struct Emitter<'m> {
    ctx: &'m SourceContext<'m>,
    /// Columns and cells of the table in this block, if it is one — so the block can
    /// also be expressed in its always-fitting form.
    table: Option<(usize, String)>,
    /// Longest cell in each column of that table, which decides who absorbs the slack.
    ///
    /// ponytail: measures the emitted markup, not the rendered text. A header's
    /// `#strong[..]` wrapper is constant per column and cancels out; a cell holding a
    /// link carries its URL and reads wider than it draws. ceiling: a narrow column that
    /// contains a long link is granted `1fr`, which costs it nothing. upgrade: count
    /// `Event::Text` while inside the cell.
    cell_widths: Vec<usize>,
    cell_index: usize,
    footnotes: &'m HashMap<String, String>,
    /// Compromises for the block being emitted; the Element id is attached once the
    /// body is known.
    pending: Vec<CompromiseKind>,
    images: ImageManifest,
    /// Table header cells render bold, as GitHub does.
    in_header: bool,
    /// Insert break opportunities into runs longer than this many characters.
    ///
    /// Set only for the second pass over a table's cells, which builds the reflow
    /// alternate. The body must keep its runs unbroken: its natural width is what the
    /// probe measures to choose a rung, and a body that could wrap mid-token would
    /// measure narrow and be given the wrong decision.
    breakable: Option<Vec<usize>>,
}

impl Emitter<'_> {
    /// Break long runs, but only when emitting the alternate.
    fn maybe_break<'t>(&self, text: &'t str) -> std::borrow::Cow<'t, str> {
        match &self.breakable {
            Some(limits) if !limits.is_empty() => {
                // Which column this cell sits in decides how hard to break.
                let limit = limits[self.cell_index % limits.len()];
                std::borrow::Cow::Owned(offer_breaks(text, limit))
            }
            Some(_) => std::borrow::Cow::Borrowed(text),
            None => std::borrow::Cow::Borrowed(text),
        }
    }

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
                Event::Text(t) => out.push_str(&escape(&self.maybe_break(t))),
                Event::Code(t) => {
                    out.push_str(&format!(
                        "#raw(\"{}\")",
                        escape_string(&self.maybe_break(t))
                    ));
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
                // Reset before the cells arrive: one table's widths must not leak into
                // the next, and a document may hold many.
                self.cell_widths = vec![0; columns];
                self.cell_index = 0;
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
                // Row-major, so the column is the position within the row.
                if !self.cell_widths.is_empty() {
                    let col = self.cell_index % self.cell_widths.len();
                    self.cell_widths[col] = self.cell_widths[col].max(inner.chars().count());
                    self.cell_index += 1;
                }
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

    /// The alternate the ladder falls back to, when a table will not fit.
    fn alternate(md: &str) -> String {
        run(md)
            .elements
            .iter()
            .find_map(|e| e.reflow.as_ref().map(|m| m.as_str().to_string()))
            .expect("the table carried no reflow alternate")
    }

    #[test]
    fn column_widths_are_proportional_to_their_content() {
        // "P1" must not be given the same width as a paragraph — the defect that made
        // deep shrinking beat reflow on every real table. See plan-reflow-columns.md.
        let a = alternate(
            "| id | note | p |\n|---|---|---|\n\
             | E01 | a considerably longer sentence that has to wrap somewhere | P1 |",
        );
        assert!(
            a.starts_with("#table(columns: (1fr, 6fr, 1fr),"),
            "wrong column spec: {a}"
        );
    }

    #[test]
    fn two_wide_columns_both_get_the_larger_share() {
        // The rule is relative, so "widest column only" is not enough: a table with two
        // prose columns needs both to absorb, or the second sizes to its full content.
        let a = alternate(
            "| id | first | second |\n|---|---|---|\n\
             | E1 | a reasonably long sentence here | another reasonably long one too |",
        );
        assert!(
            a.starts_with("#table(columns: (1fr, 6fr, 6fr),"),
            "wrong column spec: {a}"
        );
    }

    #[test]
    fn a_uniform_table_gives_every_column_a_share() {
        // Where every column is the same width the old behaviour is the right one, and
        // the alternate must still fill the width rather than collapsing to the body.
        let a = alternate("| aaa | bbb |\n|---|---|\n| ccc | ddd |");
        assert!(
            a.starts_with("#table(columns: (6fr, 6fr),"),
            "wrong column spec: {a}"
        );
    }

    #[test]
    fn every_column_is_fractional_so_the_table_cannot_exceed_the_page() {
        // The guarantee `Reflow` rests on, and the one an `auto` column broke: fr
        // columns divide the width available, so the total can never exceed it. Four
        // real tables ran off the page when narrow columns were `auto` (F8).
        for md in [
            "| a |\n|---|\n| b |",
            "| a | b | c | d |\n|---|---|---|---|\n| 1 | 2 | 3 | 4 |",
            "| tiny | enormously long cell content goes here |\n|---|---|\n| x | y |",
        ] {
            let a = alternate(md);
            let spec = a.split("), ").next().unwrap_or("");
            assert!(!spec.contains("auto"), "an auto column crept back in: {a}");
            assert!(spec.contains("fr"), "no fractional column: {a}");
        }
    }

    #[test]
    fn each_table_is_measured_on_its_own() {
        // The widths are per-table state; a wide table must not set the scale for the
        // narrow one that follows it.
        let out = run(
            "| id | note |\n|---|---|\n| E01 | a considerably longer sentence to wrap |\n\n\
             | aa | bb |\n|---|---|\n| cc | dd |",
        );
        let specs: Vec<String> = out
            .elements
            .iter()
            .filter_map(|e| e.reflow.as_ref())
            // Just the column spec, not the cells.
            .map(|m| m.as_str().split("), ").next().unwrap_or("").to_string())
            .collect();
        assert_eq!(specs.len(), 2, "expected two tables: {specs:?}");
        // The first table is lopsided, the second uniform. If widths leaked between
        // them the second would inherit the first's skew.
        assert!(
            specs[0].contains("6fr") && specs[0].contains("1fr"),
            "first: {specs:?}"
        );
        assert!(
            !specs[1].contains("1fr"),
            "second table inherited the first's widths: {specs:?}"
        );
    }

    const ZWSP: char = '\u{200b}';

    #[test]
    fn long_runs_get_break_opportunities_in_the_alternate() {
        // Inline code is 77% of the long runs in the corpus, and `#raw` renders text
        // verbatim — so this is the case that matters most.
        //
        // The run must sit in a **narrow** column to need breaking. Each column's
        // threshold comes from its own share, so the same identifier beside four
        // one-character columns takes most of the width and is left alone — correctly.
        // Here a long prose column crowds it into a small share instead.
        let prose = "a considerably longer sentence that keeps going and going so that \
                     this column earns the lion's share of the available width";
        let a = alternate(&format!(
            "| call | detail |\n|---|---|\n\
             | `completeSubmissionWithAVeryLongIdentifier` | {prose} |"
        ));
        assert!(a.contains(ZWSP), "no break offered in the alternate: {a}");
    }

    #[test]
    fn the_body_keeps_its_runs_unbroken() {
        // Load-bearing. The probe measures the *body* to choose a rung; a body that
        // could wrap mid-token would measure narrow and be given the wrong decision.
        let md = "| a | b | c | d | e |\n|---|---|---|---|---|\n\
             | `completeSubmissionWithAVeryLongIdentifier` | x | y | z | w |";
        let b = &bodies(md)[0];
        assert!(!b.contains(ZWSP), "the body was broken too: {b}");
    }

    #[test]
    fn ordinary_words_are_left_alone() {
        let a = alternate("| a | b |\n|---|---|\n| short words only | nothing long here |");
        assert!(!a.contains(ZWSP), "broke an ordinary word: {a}");
    }

    #[test]
    fn breaking_does_not_disturb_escaping() {
        // A break inserted after escaping could land inside `\#`, producing `\<zwsp>#`
        // — a backslash escaping nothing, and a stray hash. Breaks go in first.
        let a = alternate(
            "| a | b | c | d | e |\n|---|---|---|---|---|\n\
             | costs##and#more#hashes#than#you#would#like#here | x | y | z | w |",
        );
        assert!(
            !a.contains(&format!("\\{ZWSP}")),
            "break split an escape: {a}"
        );
        assert!(a.contains("\\#"), "escaping was lost: {a}");
    }

    #[test]
    fn a_second_pass_does_not_double_count_compromises() {
        // The alternate is emitted by running the block again. Its compromises and
        // images must be discarded, or one missing image in a table is reported twice.
        let out = run("| a | b |\n|---|---|\n| ![gone](nope.png) | x |");
        assert_eq!(
            out.compromises.len(),
            1,
            "compromises double-counted: {:?}",
            out.compromises
        );
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
