//! T31 — does memory actually grow across compilations, and does eviction bound it?
//!
//! **F3 has four sightings and no measurement.** Each was a process dying: the T26b
//! exposure run, the same again with a `Typesetter` per element, the T30 base-size
//! comparison, and the same with a `Typesetter` per probe. Every response so far has been
//! a workaround chosen from a guess about the cause, and two of them were placebo —
//! `comemo`'s cache is process-global, so dropping a `Typesetter` frees nothing.
//!
//! A fifth sighting is now reproducible: the release batch is OOM-killed at ~141 of 146
//! documents. This measures the curve behind it rather than inferring one from corpses.
//!
//! ```text
//! cargo test --release -p md2pdf-engine --test walking_skeleton \
//!     memory_growth_across_the_corpus -- --ignored --nocapture
//! ```

/// Resident set size in MiB, from the kernel rather than an allocator estimate.
///
/// `VmRSS` is what the OOM killer reads, which is the number that matters here — a
/// `comemo` cache that is retained but idle still counts.
pub fn rss_mib(broker: &md2pdf_paths::PathBroker) -> f64 {
    let Ok(status) = broker.read_to_string(std::path::Path::new("/proc/self/status")) else {
        return 0.0;
    };
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
        .map(|kib| kib / 1024.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_convert::{convert, SourceContext};
    use md2pdf_domain::Template;
    use md2pdf_typeset::Typesetter;

    /// Compile the corpus one document at a time, reporting RSS as it goes.
    ///
    /// Set `EVICT=<age>` to call `comemo::evict` between documents, which is the fix
    /// under test. Without it, this is the curve that ends in the OOM killer.
    #[test]
    #[ignore = "needs documents/; the F3 measurement"]
    fn memory_growth_across_the_corpus() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = Template::default();
        let evict: Option<usize> = std::env::var("EVICT").ok().and_then(|v| v.parse().ok());

        // One long-lived Typesetter, which is the shape 3f's recompile loop will have.
        let ts = Typesetter::new();
        let start = rss_mib(&broker);
        println!("start {start:.0} MiB   evict={evict:?}");

        let mut sources = broker
            .walk(&std::path::PathBuf::from("/workspace/documents"))
            .expect("corpus")
            .sources;
        sources.sort();

        for (i, source) in sources.iter().enumerate() {
            let Ok(markdown) = broker.read_to_string(source) else {
                continue;
            };
            let parent = source.parent().unwrap().to_path_buf();
            let images = super::super::census::CorpusImages;
            let conversion = convert(&markdown, &SourceContext::new(&parent, &images));

            ts.clear_files();
            for (name, path) in &conversion.images {
                if let Ok(bytes) = broker.read_bytes(path) {
                    ts.add_file(name, bytes);
                }
            }
            let Ok((map, _)) = ts.probe(&conversion.elements, &template) else {
                continue;
            };
            if let Ok(c) = ts.render(&conversion.elements, &template, &map) {
                // Producing the PDF is what the batch does; without it the peak is not
                // the batch's peak.
                let _ = c.pdf();
            }
            if let Some(age) = evict {
                md2pdf_typeset::evict(age);
            }

            if i % 10 == 0 || i + 1 == sources.len() {
                println!(
                    "  {:>3} documents  {:>6.0} MiB  (+{:.0} since start)",
                    i + 1,
                    rss_mib(&broker),
                    rss_mib(&broker) - start
                );
            }
        }
        println!("end {:.0} MiB", rss_mib(&broker));
    }

    /// What does eviction cost the recompile loop 3f is built around?
    ///
    /// The memory dial is only half the trade. `comemo` exists so that recompiling a
    /// document the user is editing costs milliseconds, and evicting too eagerly throws
    /// that away. This recompiles one real document repeatedly at several ages and
    /// reports both the time and the resident set.
    #[test]
    #[ignore = "needs documents/; the F3 trade-off"]
    fn what_eviction_costs_the_recompile_loop() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = Template::default();
        let path = std::path::Path::new("/workspace/documents/design-docs/design__event-storm.md");
        let markdown = broker.read_to_string(path).expect("source");
        let parent = path.parent().unwrap().to_path_buf();
        let images = super::super::census::CorpusImages;
        let conversion = convert(&markdown, &SourceContext::new(&parent, &images));

        println!(
            "\n{:>8}  {:>10}  {:>10}  {:>8}",
            "evict", "first", "steady", "RSS"
        );
        for age in [None, Some(100), Some(10), Some(5), Some(1), Some(0)] {
            // A fresh Typesetter does not clear the global cache, so a cold reading
            // needs the cache emptied first — which is itself a use of evict(0).
            md2pdf_typeset::evict(0);
            let ts = Typesetter::new();
            let mut first = 0u128;
            let mut steady = 0u128;
            for i in 0..6 {
                let t = std::time::Instant::now();
                let Ok((map, _)) = ts.probe(&conversion.elements, &template) else {
                    continue;
                };
                let _ = ts.render(&conversion.elements, &template, &map);
                let ms = t.elapsed().as_millis();
                if i == 0 {
                    first = ms;
                } else {
                    steady = steady.max(ms);
                }
                if let Some(a) = age {
                    md2pdf_typeset::evict(a);
                }
            }
            println!(
                "{:>8}  {:>8}ms  {:>8}ms  {:>6.0}MiB",
                age.map(|a| a.to_string()).unwrap_or("none".into()),
                first,
                steady,
                rss_mib(&broker)
            );
        }
    }

    /// **The gate: eviction still does something.**
    ///
    /// Not a memory assertion, and the reason matters. `VmRSS` is process-wide, cargo
    /// runs this binary's tests concurrently, and the allocator reuses freed pages — so
    /// an in-suite memory bound measures whatever else is compiling. Two versions of this
    /// test failed that way while the code was correct: an absolute bound at 81 MiB, then
    /// a paired comparison that collapsed to 82-vs-92 because the first half left the
    /// allocator warm for the second. **The memory curve is real and measured — it is
    /// just not measurable from inside a concurrent test binary.** It lives in
    /// `memory_growth_across_the_corpus`, run deliberately; see `design/plan-comemo.md`.
    ///
    /// What *is* robust is whether eviction has any effect at all, and that is the failure
    /// this needs to catch. F3's two placebo fixes both looked like they worked; the
    /// version pin on `comemo` guards a skew that would silently stop evicting anything.
    /// A cleared cache is **[measured]** ~100x slower to recompile — 470ms against 4ms —
    /// so the signal survives any amount of machine noise.
    #[test]
    fn eviction_still_evicts() {
        let template = Template::default();
        let conversion = convert(&synthetic(0), &SourceContext::none());
        let ts = Typesetter::new();

        let once = |ts: &Typesetter| {
            let t = std::time::Instant::now();
            if let Ok((map, _)) = ts.probe(&conversion.elements, &template) {
                let _ = ts.render(&conversion.elements, &template, &map);
            }
            t.elapsed().as_micros()
        };

        once(&ts); // warm: everything this document needs is now memoised
        let cached = once(&ts).max(1);
        md2pdf_typeset::evict(0);
        let cleared = once(&ts);

        assert!(
            cleared > cached * 5,
            "clearing the cache changed nothing ({cleared}us after evict(0) vs {cached}us \
             cached) — `comemo::evict` is not reaching the cache typst memoises through. \
             Check that the `comemo` version pin still matches typst's."
        );
    }

    #[test]
    #[ignore = "calibration for the gate above"]
    fn calibrate_the_bound() {
        let broker = md2pdf_paths::PathBroker::new();
        for age in [None, Some(5)] {
            md2pdf_typeset::evict(0);
            let before = rss_mib(&broker);
            let ts = Typesetter::new();
            let template = Template::default();
            for i in 0..40 {
                let md = synthetic(i);
                let conversion = convert(&md, &SourceContext::none());
                if let Ok((map, _)) = ts.probe(&conversion.elements, &template) {
                    let _ = ts.render(&conversion.elements, &template, &map);
                }
                if let Some(a) = age {
                    md2pdf_typeset::evict(a);
                }
            }
            println!("evict={age:?}  grew {:.0} MiB", rss_mib(&broker) - before);
        }
    }

    /// Each document distinct, so `comemo` cannot reuse the last one's entries — which is
    /// what makes a corpus grow where recompiling one file does not.
    fn synthetic(i: usize) -> String {
        let mut md = format!("# Document {i}\n\nParagraph {i} with some prose in it.\n\n");
        for r in 0..12 {
            md.push_str(&format!(
                "| id{i}_{r} | value {i}.{r} | a sentence unique to row {r} of document {i} |\n"
            ));
            if r == 0 {
                md.push_str("|---|---|---|\n");
            }
        }
        md
    }
}
