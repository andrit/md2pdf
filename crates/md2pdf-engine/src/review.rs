//! The review loop — hold one document open, and let a person revisit a judgment call.
//!
//! The payoff for treating layout as something that *emits*. Everything before this
//! recorded what md2pdf conceded; this is the first thing that offers to change it.
//!
//! ## What is held, and what is redone
//!
//! | | On an Override |
//! |---|---|
//! | the Conversion — parse, classify, emit | **kept** — the markdown has not changed |
//! | the ProbePass — measure and decide | **redone**, under the new permission |
//! | the RenderPass | redone |
//!
//! Re-parsing is the expensive part `comemo` cannot help with, because it happens before
//! Typst is involved at all. Re-probing is what an Override *means*: a permission changes
//! the ladder's constraints, and the size that comes back is measured under them rather
//! than inherited. See `md2pdf_domain::review`.
//!
//! ## This is the loop that kept dying
//!
//! A long-lived `Typesetter` recompiling repeatedly is the exact shape that was
//! OOM-killed five times before T31 found that nothing ever evicted the `comemo` cache.
//! **It deliberately does not evict per recompile**: here the previous compilation still
//! being cached is the entire point, and **[measured]** `evict(0)` makes a recompile ~100x
//! slower. The batch evicts between *documents*; a review session holds one. See
//! `design/plan-comemo.md` D2.

use md2pdf_convert::{convert, Conversion, SourceContext};
use md2pdf_domain::{
    AttentionList, DecisionMap, Diagnostic, Element, ElementId, Override, Template,
};
use md2pdf_typeset::{Compilation, Typesetter};

use crate::job::JobError;

/// One document, open for review.
pub struct Review {
    conversion: Conversion,
    template: Template,
    decisions: DecisionMap,
    diagnostic: Diagnostic,
    overrides: Vec<Override>,
}

impl Review {
    /// Convert and probe once. Everything after this is a recompile.
    pub fn open(
        markdown: &str,
        context: &SourceContext,
        template: Template,
        typesetter: &Typesetter,
    ) -> Result<Self, JobError> {
        let conversion = convert(markdown, context);
        // The probe's own Diagnostic is discarded: `seal` rebuilds it from the same
        // decisions *plus* the conversion's half, and only a sealed one is complete
        // (`INV-4`).
        let (decisions, _) = typesetter
            .probe(&conversion.elements, &template)
            .map_err(|e| JobError::Compile(e.to_string()))?;
        Ok(Self {
            diagnostic: Diagnostic::seal(conversion.compromises.clone(), &decisions),
            conversion,
            template,
            decisions,
            overrides: Vec::new(),
        })
    }

    /// What needed a judgment call, and what can be offered instead.
    pub fn attention(&self) -> AttentionList {
        AttentionList::from_diagnostic(&self.diagnostic)
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    /// Every Override in force, in the order they were applied.
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }

    /// Apply a permission and re-decide under it.
    ///
    /// **Replaces rather than accumulates for the same Element**: two permissions for one
    /// Element are a person changing their mind, not both applying at once — and keeping
    /// both would make the result depend on the order they were clicked in.
    ///
    /// Returns `false` if the Override names an Element this document does not have,
    /// which is how a stale one from an edited Source is discarded rather than misapplied.
    pub fn apply(&mut self, over: Override, typesetter: &Typesetter) -> Result<bool, JobError> {
        if !self
            .conversion
            .elements
            .iter()
            .any(|e| e.id.matches(&over.id))
        {
            return Ok(false);
        }
        self.overrides.retain(|o| !o.id.matches(&over.id));
        self.overrides.push(over);
        self.refresh(&[over.id], typesetter)?;
        Ok(true)
    }

    /// Apply several permissions and re-decide **once**.
    ///
    /// What the grouped attention list clicks: "give all five of these a landscape page"
    /// is one intention, and running it as five `apply` calls would re-probe five times
    /// and emit four intermediate decisions nobody asked to see. `refresh` already takes
    /// a set of ids, so the whole row costs one probe.
    ///
    /// Returns how many of the Overrides named an Element this document still has.
    /// **Zero is not an error** and is reported as such by the caller — it means every
    /// click was stale, which is the same condition [`apply`](Self::apply) returns
    /// `false` for.
    pub fn apply_all(
        &mut self,
        overs: &[Override],
        typesetter: &Typesetter,
    ) -> Result<usize, JobError> {
        let live: Vec<Override> = overs
            .iter()
            .filter(|o| self.conversion.elements.iter().any(|e| e.id.matches(&o.id)))
            .copied()
            .collect();
        if live.is_empty() {
            return Ok(0);
        }
        for over in &live {
            // Replaces rather than accumulates, for the same reason `apply` does.
            self.overrides.retain(|o| !o.id.matches(&over.id));
            self.overrides.push(*over);
        }
        let ids: Vec<_> = live.iter().map(|o| o.id).collect();
        self.refresh(&ids, typesetter)?;
        Ok(live.len())
    }

    /// Withdraw a permission and re-decide without it.
    pub fn withdraw(&mut self, over: &Override, typesetter: &Typesetter) -> Result<(), JobError> {
        self.overrides.retain(|o| !o.id.matches(&over.id));
        // Still refreshed: without its permission the Element has to be decided again,
        // and leaving the overridden decision in place is how "undo" silently does nothing.
        self.refresh(&[over.id], typesetter)
    }

    /// Re-decide only the Elements whose permissions changed, and splice the results in.
    ///
    /// **[measured] this is the difference between 1567ms and something usable.** The
    /// ProbePass builds *one* Typst document containing the measurement code for every
    /// Element, so changing one Element's constants changes the whole source — and
    /// `comemo` keys on the source. Re-probing the document to move one table therefore
    /// re-measures all hundred of them, and a 100-element document took 1.5 seconds per
    /// click. The plan assumed under 100ms from T31's 4ms recompile figure; that figure
    /// was for recompiling the *same* source, where every measurement is a cache hit.
    ///
    /// Probing the changed Elements alone is only sound because their decisions do not
    /// depend on each other — pinned by `probing_one_element_agrees_with_probing_the_
    /// document` in `md2pdf-typeset`'s contract tests, which exists for exactly this.
    fn refresh(&mut self, ids: &[ElementId], typesetter: &Typesetter) -> Result<(), JobError> {
        let subset: Vec<Element> = self
            .conversion
            .elements
            .iter()
            .filter(|e| ids.iter().any(|id| id.matches(&e.id)))
            .cloned()
            .collect();
        if subset.is_empty() {
            return Ok(());
        }
        let (fresh, _) = typesetter
            .probe_with(&subset, &self.template, &self.overrides)
            .map_err(|e| JobError::Compile(e.to_string()))?;
        for decision in fresh.decisions {
            self.decisions.replace(decision);
        }
        self.diagnostic = Diagnostic::seal(self.conversion.compromises.clone(), &self.decisions);
        Ok(())
    }

    /// The document as currently decided.
    pub fn render(&self, typesetter: &Typesetter) -> Result<Compilation, JobError> {
        typesetter
            .render(&self.conversion.elements, &self.template, &self.decisions)
            .map_err(|e| JobError::Compile(e.to_string()))
    }

    pub fn decisions(&self) -> &DecisionMap {
        &self.decisions
    }
}
