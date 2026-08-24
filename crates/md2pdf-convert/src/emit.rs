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
            cell_min: Vec::new(),
            longest: Vec::new(),
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
                // The alternate's cells are emitted a *second* time, with breaking on.
                // Re-running the same code path rather than rewriting the first pass's
                // markup: both forms then cannot drift, and nothing has to parse Typst
                // to find where a break may safely go.
                // Two passes, and the second almost never runs. The first offers breaks
                // only to tokens that fit nowhere at all, which is what keeps ordinary
                // words whole. It then asks whether the minimums it discovered can
                // actually coexist: if the columns' longest tokens cannot all fit side
                // by side, no allocation can satisfy them and *something* has to break,
                // so the table is emitted again with each column held to its share.
                //
                // **[measured]** `below-comfort-reflows.md` is the shape that needs it —
                // six columns of a 22-character run, wanting 798pt of a 423pt table.
                let usable = ctx.template.available_pt() - columns as f64 * 2.0 * TABLE_INSET_PT;
                let across = ctx.template.chars_in(usable);
                let generous = vec![across.max(4); columns];
                let (cells, mins) =
                    breakable_cells(block, ctx, &footnotes, generous).unwrap_or_default();

                let demand: usize = mins.iter().map(|m| m.chars().count()).sum();
                let (cells, mins) = if demand > across {
                    breakable_cells(block, ctx, &footnotes, crowded_limits(&mins, across))
                        .unwrap_or_default()
                } else {
                    (cells, mins)
                };
                Element::with_reflow(
                    order,
                    block.class,
                    Markup::raw(body),
                    Markup::raw(self_sizing_table(columns, &cells, &mins)),
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
    limits: Vec<usize>,
) -> Option<(String, Vec<String>)> {
    let mut em = Emitter {
        ctx,
        table: None,
        cell_min: Vec::new(),
        longest: Vec::new(),
        cell_index: 0,
        footnotes,
        pending: Vec::new(),
        images: ImageManifest::new(),
        in_header: false,
        breakable: Some(limits),
    };
    em.sequence(&mut block.events.iter().peekable(), None);
    let mins = em.cell_min.clone();
    em.table.take().map(|(_, cells)| (cells, mins))
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
            cell_min: Vec::new(),
            longest: Vec::new(),
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

/// Share the table between columns that cannot all have what they want.
///
/// Only reached when a table is **unfittable as written**: its columns' longest
/// unbreakable tokens will not go side by side however the width is divided, so a token
/// must break or the table runs off the page. `below-comfort-reflows.md` is the shape —
/// six columns each holding a 22-character run, asking 798pt of a 423pt table.
///
/// **Cap the greedy, leave the rest alone.** The share is *not* proportional: dividing
/// the width in proportion to demand punishes every column for one column's hash, and
/// **[measured]** that alone still broke 103 ordinary words across the corpus. Instead
/// each column is capped at the largest ceiling under which the table fits, so a column
/// whose longest token is already below that ceiling is never touched at all. A table of
/// four eight-character words beside one sixty-character hash caps at 38 — the hash
/// breaks, the words do not; proportional sharing would have given the words six.
///
/// This is the one place a width is still inferred from a character count, and it is
/// deliberately the *last*: erring here changes a table that was going to be crowded
/// either way, where erring in the common path shredded `Executio|n`.
fn crowded_limits(mins: &[String], across: usize) -> Vec<usize> {
    let mut lens: Vec<usize> = mins.iter().map(|m| m.chars().count()).collect();
    lens.sort_unstable();

    // Water-filling: walk the columns from narrowest to widest, letting each take what
    // it asks for while the rest could still be served an equal share of the remainder.
    // The first column that cannot is where the ceiling falls.
    let mut remaining = across;
    let mut cap = lens.last().copied().unwrap_or(across);
    for (i, len) in lens.iter().enumerate() {
        let left = lens.len() - i;
        if *len * left <= remaining {
            remaining -= *len;
        } else {
            cap = remaining / left;
            break;
        }
    }
    // Never below the floor `break_word` counts to.
    vec![cap.max(4); mins.len()]
}

/// The reflow alternate: a table that measures its own contents and sizes its own
/// columns, at layout time, inside the document.
///
/// `convert` cannot know how wide text will be — it has no fonts and no layout engine —
/// and four consecutive defects (T29, T29b, T29c, T30) came from it estimating anyway.
/// Typst knows exactly. So each side does what it can: **`convert` supplies the words,
/// Typst supplies the widths.**
///
/// ## The allocation
///
/// Min-content first, which is the standard table algorithm and what browsers do:
///
/// 1. every column takes at least `mins[i]` — its longest unbreakable run, measured;
/// 2. what is left over is shared in proportion to how much *more* each column wants.
///
/// A column is therefore never narrower than a word it must display, so Typst is never
/// forced to break one. That is the whole defect, stated as an invariant.
///
/// ## Why `fr` rather than the computed points
///
/// The widths are converted back into fractions. Fractional columns **divide** the space
/// available, so the total cannot exceed the page whatever the arithmetic does — the
/// guarantee `Reflow` depends on to sit one rung before `Clip` (T26a2). The points only
/// set the *proportions*; Typst still enforces the total.
///
/// ## The fallback
///
/// When the minimums alone exceed the usable width the table genuinely cannot be laid
/// out without breaking something, and the shares go back to being proportional to
/// demand. Tokens keep the break opportunities [`break_limits`] gave them.
fn self_sizing_table(columns: usize, cells: &str, mins: &[String]) -> String {
    let min_cells: String = (0..columns)
        .map(|i| {
            // Already markup — `note_unbreakable` escaped it in the face it will be
            // drawn in, so escaping again here would show the escapes.
            format!("[{}], ", mins.get(i).map(String::as_str).unwrap_or(""))
        })
        .collect();
    let inset = 2.0 * TABLE_INSET_PT;
    format!(
        "#layout(size => {{\n\
         let cs = ({cells})\n\
         let mn = ({min_cells})\n\
         let n = {columns}\n\
         let usable = size.width - {inset}pt * n\n\
         let mins = mn.map(t => measure(t).width)\n\
         let dem = range(n).map(i => range(cs.len())\n\
           .filter(k => calc.rem(k, n) == i)\n\
           .map(k => measure(cs.at(k)).width)\n\
           .fold(0pt, (a, b) => calc.max(a, b)))\n\
         let mtot = mins.fold(0pt, (a, b) => a + b)\n\
         let dtot = dem.fold(0pt, (a, b) => a + b)\n\
         let w = if mtot >= usable or dtot == 0pt {{\n\
           if dtot == 0pt {{ range(n).map(_ => usable / n) }}\n\
           else {{ dem.map(d => usable * (d / dtot)) }}\n\
         }} else {{\n\
           let extra = range(n).map(i => calc.max(dem.at(i) - mins.at(i), 0pt))\n\
           let etot = extra.fold(0pt, (a, b) => a + b)\n\
           let spare = usable - mtot\n\
           if etot == 0pt {{ mins.map(m => m + spare / n) }}\n\
           else {{ range(n).map(i => mins.at(i) + spare * (extra.at(i) / etot)) }}\n\
         }}\n\
         table(columns: w.map(x => ((x + {inset}pt) / 1pt) * 1fr), ..cs)\n\
         }})"
    )
}

/// What a table cell loses to padding, on each side.
///
/// **[measured]** 2026-08-23 by rendering a one-column table and finding where the glyph
/// ink begins: 5pt right of the border. It is Typst's default and the emitted `#table`
/// does not override it, so `convert` — which writes that call — is where the assumption
/// belongs.
///
/// It matters because it is charged **per column**: a twelve-column table spends 120 of
/// its 483 points on inset, a quarter of the width, and a limit computed as though the
/// whole width held text is a quarter too generous. That is what pushed
/// `reflow-hostile.md` 4pt off the page at a 12pt base while passing at 10pt.
const TABLE_INSET_PT: f64 = 5.0;

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
    let mut out = String::with_capacity(text.len());
    // Judged per word: a short one must not be split just because it follows a long one.
    for word in text.split_inclusive(char::is_whitespace) {
        if word.trim_end().chars().count() <= limit {
            out.push_str(word);
        } else {
            break_word(word, limit, &mut out);
        }
    }
    out
}

/// Characters a reader would already break a long token at.
///
/// Breaking after one of these gives `user_ / organization_ / roles`, which is how the
/// name reads anyway. Counting characters instead gave `user_organiz / ation_roles`,
/// visible in `design/evidence/t26c/pair-8-5pt-p0.png` and recorded as flag **F9**.
const SEPARATORS: [char; 7] = ['_', '.', '/', '-', ':', '\\', ','];

/// Insert break opportunities into one over-long word.
///
/// Separators first, counting only as a fallback — an identifier or a path almost always
/// has one, and a hash or a long word does not.
fn break_word(word: &str, limit: usize, out: &mut String) {
    // A full column's worth, not half of one. `limit` is how many characters the column
    // has room for, so counting to it breaks exactly when the line is full — while
    // anything shorter chops words that would have fitted. At `limit / 2` this produced
    // `user_organi|zation_roles` even though `organization_` fits on its own.
    let every = limit.max(4);
    let mut run = 0usize;
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c.is_whitespace() {
            run = 0;
            continue;
        }
        run += 1;
        // Never offer a break at the very end of a word: it can only produce a line
        // ending one character early.
        if chars.peek().is_none_or(|n| n.is_whitespace()) {
            continue;
        }
        if SEPARATORS.contains(&c) || run >= every {
            out.push(BREAK);
            run = 0;
        }
    }
}

struct Emitter<'m> {
    ctx: &'m SourceContext<'m>,
    /// Columns and cells of the table in this block, if it is one — so the block can
    /// also be expressed in its always-fitting form.
    table: Option<(usize, String)>,
    /// The longest **unbreakable** run in each column: the widest thing Typst will not
    /// be able to wrap, and therefore the narrowest that column may ever be.
    ///
    /// Recorded as text rather than a count, because a count is a width estimate and
    /// four defects came from those. `convert` knows the words; `typeset` knows how wide
    /// they are — so this is emitted for Typst to measure. See `plan-typeset-move.md`.
    cell_min: Vec<String>,
    /// Length in characters of each entry in `cell_min` — kept beside it because that
    /// field holds markup, whose length is not the length of the text it draws.
    longest: Vec<usize>,
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
    fn maybe_break<'t>(&mut self, text: &'t str, code: bool) -> std::borrow::Cow<'t, str> {
        match &self.breakable {
            Some(limits) if !limits.is_empty() => {
                let limit = limits[self.cell_index % limits.len()];
                let broken = offer_breaks(text, limit);
                self.note_unbreakable(&broken, code);
                std::borrow::Cow::Owned(broken)
            }
            Some(_) => std::borrow::Cow::Borrowed(text),
            None => std::borrow::Cow::Borrowed(text),
        }
    }

    /// Remember this column's longest run with nowhere to wrap.
    ///
    /// Measured **after** breaking, so a token that has been given opportunities counts
    /// as its longest *segment* rather than its whole length — otherwise a hash would
    /// demand a column as wide as itself when it is perfectly happy to wrap.
    ///
    /// Stored as **markup, not text**, so Typst measures it in the face it will actually
    /// be drawn in. Inline code renders in the mono family, which is wider than the body
    /// at the same size: measuring `integration.destination.verified` as plain text made
    /// its column a little too narrow and the token overlapped the cell border — visible
    /// in the T26c 9.0pt pair, and invisible to every count.
    fn note_unbreakable(&mut self, broken: &str, code: bool) {
        if self.cell_min.is_empty() {
            return;
        }
        let col = self.cell_index % self.cell_min.len();
        for run in broken.split([BREAK, ' ', '\t', '\n']) {
            if run.chars().count() > self.longest[col] {
                self.longest[col] = run.chars().count();
                self.cell_min[col] = if code {
                    format!("#raw(\"{}\")", escape_string(run))
                } else {
                    escape(run)
                };
            }
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
                Event::Text(t) => out.push_str(&escape(&self.maybe_break(t, false))),
                Event::Code(t) => {
                    out.push_str(&format!(
                        "#raw(\"{}\")",
                        escape_string(&self.maybe_break(t, true))
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
                self.cell_min = vec![String::new(); columns];
                self.longest = vec![0; columns];
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
                if !self.cell_min.is_empty() {
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
    fn a_crowded_table_shares_what_it_cannot_all_have() {
        // The fallback, and the only place a width is still guessed from a count. Six
        // columns each wanting a 22-character run cannot go side by side in a 423pt
        // table, so each is held to its share rather than overflowing the page.
        let mins: Vec<String> = std::iter::repeat_n("x".repeat(22), 6).collect();
        assert_eq!(
            crowded_limits(&mins, 70),
            vec![11; 6],
            "equal columns share equally"
        );

        // The point of the cap: one greedy column must not cost the others their words.
        // Four eight-character words beside a sixty-character hash fit if the hash is
        // held to 38 — and at 38 the words are untouched, because `offer_breaks` only
        // acts on a token longer than the limit. Sharing in proportion would give the
        // word columns six characters and shred every one of them.
        let mut mixed = vec!["x".repeat(60)];
        mixed.extend(std::iter::repeat_n("x".repeat(8), 4));
        let cap = crowded_limits(&mixed, 70)[0];
        assert_eq!(cap, 38, "the cap should fall on the hash alone");
        assert!(
            cap > 8,
            "an eight-character word would still be broken at {cap}"
        );

        // Never below the floor `break_word` counts to, however lopsided.
        assert!(crowded_limits(&["x".to_string(), "x".repeat(500)], 20)[0] >= 4);
    }

    #[test]
    fn an_ordinary_word_is_never_broken_however_narrow_its_column() {
        // The defect this task exists for. 579 of the 1291 tokens broken across the
        // corpus were ordinary words. A twelve-column table is the most crowded shape
        // there is, and `Execution` must still come through whole.
        let head = "| a | b | c | d | e | f | g | h | i | j | k | l |";
        let rule = "|---|---|---|---|---|---|---|---|---|---|---|---|";
        let row =
            "| Execution | Conformist | Analytics | Integration | x | x | x | x | x | x | x | x |";
        let a = alternate(&format!("{head}\n{rule}\n{row}"));
        for word in ["Execution", "Conformist", "Analytics", "Integration"] {
            assert!(a.contains(word), "{word} was broken up: {a}");
        }
    }

    #[test]
    fn a_column_may_not_be_narrower_than_its_longest_word() {
        // The invariant that makes the above safe: Typst is never *forced* to break a
        // word, because every column asks for at least its longest unbreakable run.
        // Emitted for Typst to measure rather than estimated here — that estimate is
        // what T29/T29b/T29c/T30 each got wrong.
        let a = alternate("| h | detail |\n|---|---|\n| Internationalisation | x |");
        assert!(
            a.contains("let mn = ") && a.contains("Internationalisation"),
            "no per-column minimum emitted: {a}"
        );
        assert!(
            a.contains("calc.max(dem.at(i) - mins.at(i)"),
            "minimums are emitted but not allocated first: {a}"
        );
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
    fn narrow_columns_do_not_get_a_prose_columns_width() {
        // "P1" must not be given the same width as a paragraph — the defect that made
        // deep shrinking beat reflow on every real table (plan-reflow-columns.md).
        //
        // Asserted through the *mechanism* now rather than a literal spec string. Until
        // T31a the proportions were computed here, from character counts, and could be
        // read straight out of `columns: (1fr, 6fr, 1fr)`. They are now computed by
        // Typst from measured content, so what this can check is that the table asks the
        // right question: demand per column, and a minimum under each.
        let a = alternate(
            "| id | note | p |\n|---|---|---|\n\
             | E01 | a considerably longer sentence that has to wrap somewhere | P1 |",
        );
        assert!(
            a.contains("let dem = "),
            "no per-column demand measured: {a}"
        );
        assert!(
            a.contains("let mins = "),
            "no per-column minimum measured: {a}"
        );
        // Three columns, three minimums, and the narrow ones are named in it.
        assert!(a.contains("let n = 3"), "wrong column count: {a}");
        assert!(
            a.contains("[P1], "),
            "the narrow column has no minimum: {a}"
        );
    }

    #[test]
    fn the_table_cannot_exceed_the_page() {
        // The guarantee `Reflow` rests on, and the one an `auto` column broke: fractional
        // columns *divide* the width available, so the total can never exceed it. Four
        // real tables ran off the page when narrow columns were `auto` (F8).
        //
        // T31a computes the proportions from measured widths, but deliberately hands
        // Typst `fr` rather than the points it computed — so an arithmetic mistake in the
        // allocation can misplace the proportions and still not overflow the page.
        for md in [
            "| a |\n|---|\n| b |",
            "| a | b | c | d |\n|---|---|---|---|\n| 1 | 2 | 3 | 4 |",
            "| tiny | enormously long cell content goes here |\n|---|---|\n| x | y |",
        ] {
            let a = alternate(md);
            assert!(!a.contains("auto"), "an auto column crept back in: {a}");
            assert!(
                a.contains("table(columns: w.map(x => ((x + 10pt) / 1pt) * 1fr)"),
                "the columns are not fractional: {a}"
            );
        }
    }

    #[test]
    fn each_table_is_measured_on_its_own() {
        // Per-table state: a wide table must not set the scale for the narrow one after
        // it. The minimums are the state now, so that is what must not leak.
        let out = run(
            "| id | note |\n|---|---|\n| E01 | a considerably longer sentence to wrap |\n\n\
             | aa | bb |\n|---|---|\n| cc | dd |",
        );
        let alts: Vec<String> = out
            .elements
            .iter()
            .filter_map(|e| e.reflow.as_ref())
            .map(|m| m.as_str().to_string())
            .collect();
        assert_eq!(alts.len(), 2, "expected two tables");
        assert!(alts[0].contains("sentence"), "first lost its minimum");
        assert!(
            !alts[1].contains("sentence"),
            "second table inherited the first's minimums: {}",
            alts[1]
        );
    }

    const ZWSP: char = '\u{200b}';

    /// A token no column could ever hold: longer than the whole table's usable width.
    ///
    /// These fixtures were sized against the *old* threshold, which was one column's
    /// share — a 41-character identifier beside a prose column qualified. Since T31a the
    /// threshold is the whole table, because Typst breaks at spaces on its own and the
    /// only token needing help is one that fits nowhere. So the fixtures grew, and their
    /// intent is unchanged: what genuinely cannot fit must still be given somewhere to
    /// wrap, or the table runs off the page.
    const UNFITTABLE: &str =
        "completeSubmissionWithAVeryLongIdentifierThatKeepsGoingAndGoingWellPastAnyPageWidth";

    #[test]
    fn long_runs_get_break_opportunities_in_the_alternate() {
        // Inline code is 77% of the long runs in the corpus, and `#raw` renders text
        // verbatim — so this is the case that matters most.
        let a = alternate(&format!(
            "| call | detail |\n|---|---|\n| `{UNFITTABLE}` | notes |"
        ));
        assert!(a.contains(ZWSP), "no break offered in the alternate: {a}");
    }

    #[test]
    fn breaks_land_on_separators_when_a_word_has_them() {
        // `user_organiz|ation_roles` was what counting produced (F9). A reader breaks
        // such a name after the underscore, and so should we.
        //
        // Asserted as a property rather than an exact run of text: how much *else* gets
        // broken has now depended on the base size (T30) and on the threshold (T31a), and
        // pinning the whole run made this a hostage to both. What must hold either way is
        // the F9 claim itself: **no separator is passed over.**
        let path = "documents/workbench-docs/deeply/nested/path/to/another/directory/holding/a/file_with_a_long_name.md";
        let a = alternate(&format!(
            "| table | purpose |\n|---|---|\n| {path} | notes |"
        ));
        // Count the path's own separators, not every `/` in the output — the emitted
        // `#layout` block contains division operators of its own.
        let seps = path.matches('/').count();
        assert_eq!(
            a.matches(&format!("/{ZWSP}")).count(),
            seps,
            "a separator was passed over: {a}"
        );
    }

    #[test]
    fn a_word_with_no_separator_still_gets_broken() {
        // The fallback. A hash has nothing to break at, and must still be given somewhere
        // to wrap or the table runs off the page.
        let hash = "deadbeefcafef00dbaadf00ddefaced1deadbeefcafef00dbaadf00ddefaced1cafebabedeadbeefcafef00d";
        let a = alternate(&format!(
            "| hash | purpose |\n|---|---|\n| {hash} | notes |"
        ));
        assert!(a.contains(ZWSP), "no fallback break offered: {a}");
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
