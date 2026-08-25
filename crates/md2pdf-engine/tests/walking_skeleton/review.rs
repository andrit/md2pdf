//! 3f — the attention gate and the recompile loop, exercised end to end.

#[cfg(test)]
mod tests {
    use md2pdf_convert::SourceContext;
    use md2pdf_domain::{CompromiseKind, Override, Permit, Reduction, Template};
    use md2pdf_engine::review::Review;
    use md2pdf_typeset::Typesetter;

    /// A document whose table cannot fit, so the ladder concedes something.
    const WIDE: &str = "# Report\n\nSome prose.\n\n\
        | column one | column two | column three | column four | column five |\n\
        |---|---|---|---|---|\n\
        | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx \
        | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx |\n";

    fn open(ts: &Typesetter) -> Review {
        Review::open(WIDE, &SourceContext::none(), Template::default(), ts).expect("open")
    }

    #[test]
    fn the_attention_list_names_what_was_done_and_what_is_offered() {
        let ts = Typesetter::new();
        let review = open(&ts);
        let list = review.attention();

        assert!(!list.is_empty(), "a table that cannot fit conceded nothing");
        let item = list
            .items
            .iter()
            .find(|a| matches!(a.what, CompromiseKind::Reflowed))
            .expect("the table should have reflowed");
        assert!(
            item.offers.iter().any(|o| o.permit == Permit::Landscape),
            "no landscape offered for a reflowed table"
        );
        assert!(
            !item.offers[0].label.is_empty(),
            "an offer with no words is not an offer"
        );
    }

    #[test]
    fn an_override_changes_the_decision_and_the_rendered_page() {
        // Exit criterion 2. The rendered bytes are the check: a decision that changed
        // without changing the output would be a decision that did nothing.
        let ts = Typesetter::new();
        let mut review = open(&ts);

        let before_pdf = review.render(&ts).expect("render").pdf().expect("pdf");
        let target = review
            .attention()
            .items
            .iter()
            .find(|a| matches!(a.what, CompromiseKind::Reflowed))
            .map(|a| a.id)
            .expect("a reflowed element");

        assert!(review
            .apply(
                Override {
                    id: target,
                    permit: Permit::Landscape
                },
                &ts
            )
            .expect("apply"));

        let after = review.decisions().get(&target).expect("decided");
        assert_eq!(
            after.orientation,
            md2pdf_domain::Orientation::Landscape,
            "the permission was not honoured"
        );
        assert_ne!(
            before_pdf,
            review.render(&ts).expect("render").pdf().expect("pdf"),
            "the override changed the decision but not the page"
        );
    }

    #[test]
    fn an_override_can_be_withdrawn() {
        // A review is a conversation, not a one-way door.
        let ts = Typesetter::new();
        let mut review = open(&ts);
        let target = review
            .attention()
            .items
            .iter()
            .find(|a| matches!(a.what, CompromiseKind::Reflowed))
            .map(|a| a.id)
            .expect("a reflowed element");
        let over = Override {
            id: target,
            permit: Permit::Landscape,
        };
        let before = review.decisions().get(&target).cloned().expect("decided");

        review.apply(over, &ts).expect("apply");
        review.withdraw(&over, &ts).expect("withdraw");

        assert_eq!(
            review.decisions().get(&target),
            Some(&before),
            "withdrawing did not restore the ladder's own decision"
        );
    }

    #[test]
    fn two_permissions_for_one_element_are_a_change_of_mind_not_a_stack() {
        // Otherwise the result depends on the order they were clicked in.
        let ts = Typesetter::new();
        let mut review = open(&ts);
        let target = review
            .attention()
            .items
            .iter()
            .find(|a| matches!(a.what, CompromiseKind::Reflowed))
            .map(|a| a.id)
            .expect("a reflowed element");

        review
            .apply(
                Override {
                    id: target,
                    permit: Permit::Landscape,
                },
                &ts,
            )
            .expect("apply");
        review
            .apply(
                Override {
                    id: target,
                    permit: Permit::BelowFloor { to_pt: 5.0 },
                },
                &ts,
            )
            .expect("apply");

        assert_eq!(review.overrides().len(), 1, "the permissions accumulated");
        let d = review.decisions().get(&target).expect("decided");
        assert!(
            matches!(d.reduction, Reduction::Shrink { .. }),
            "the second permission did not replace the first: {d:?}"
        );
    }

    #[test]
    fn an_override_for_an_element_this_document_does_not_have_is_refused() {
        // Exit criterion 5: a stale Override from an edited Source is discarded rather
        // than applied to whatever now occupies that position.
        let ts = Typesetter::new();
        let mut review = open(&ts);
        let before = review.decisions().clone();

        let applied = review
            .apply(
                Override {
                    id: md2pdf_domain::ElementId::new(0, "content that is not in this document"),
                    permit: Permit::Landscape,
                },
                &ts,
            )
            .expect("apply");

        assert!(!applied, "a stale Override reported success");
        assert_eq!(review.decisions(), &before, "it changed something anyway");
        assert!(review.overrides().is_empty());
    }

    /// **Exit criterion 3: does an Override feel immediate?**
    ///
    /// "Fast enough to feel immediate" is the criterion, so it gets a number rather than
    /// an opinion. ~100ms is the usual threshold for a response that reads as instant.
    ///
    /// Measured on a real corpus document rather than the fixture above, because the cost
    /// scales with element count and a five-row table would flatter it.
    #[test]
    #[ignore = "needs documents/; the 3f latency measurement"]
    fn how_long_does_an_override_take() {
        let broker = md2pdf_paths::PathBroker::new();
        let path = std::path::Path::new("/workspace/documents/design-docs/design__event-storm.md");
        let markdown = broker.read_to_string(path).expect("source");
        let parent = path.parent().unwrap().to_path_buf();
        let images = super::super::census::CorpusImages;
        let ts = Typesetter::new();

        let t0 = std::time::Instant::now();
        let mut review = Review::open(
            &markdown,
            &SourceContext::new(&parent, &images),
            Template::default(),
            &ts,
        )
        .expect("open");
        let opened = t0.elapsed().as_millis();

        let list = review.attention();
        let target = list
            .actionable()
            .next()
            .map(|a| a.id)
            .expect("something to override");

        // **Two numbers, because they measure different clicks.**
        //
        // `comemo` keys on the probe source, and each *distinct* permit produces a
        // distinct source — so the first use of an option is a cold measurement and every
        // later use of it is a cache hit. What a person feels is both: a pause the first
        // time they try landscape, then nothing when they toggle back and forth.
        let options = [Permit::Landscape, Permit::BelowFloor { to_pt: 5.0 }];

        let mut cold = 0u128;
        for permit in options {
            let t = std::time::Instant::now();
            review
                .apply(Override { id: target, permit }, &ts)
                .expect("apply");
            let _ = review.render(&ts).expect("render").pdf().expect("pdf");
            cold = cold.max(t.elapsed().as_millis());
        }

        let mut steady = 0u128;
        for i in 0..6 {
            let permit = options[i % options.len()];
            let t = std::time::Instant::now();
            review
                .apply(Override { id: target, permit }, &ts)
                .expect("apply");
            let _ = review.render(&ts).expect("render").pdf().expect("pdf");
            steady = steady.max(t.elapsed().as_millis());
        }

        println!(
            "\n{} elements | open {opened}ms | first use of an option {cold}ms | \
             thereafter {steady}ms",
            review.decisions().decisions.len()
        );
        // The criterion is "fast enough to feel immediate" while working through the
        // list, which is the steady state. The first click is a one-off cost and is
        // reported rather than asserted on — hiding it behind an average would be the
        // dishonest version of this measurement.
        assert!(
            steady < 100,
            "an override took {steady}ms — over the 100ms that reads as immediate"
        );
    }
}
