//! `Command` / `Event` — the whole public surface of the engine.
//!
//! Plain serializable data: no lifetimes, no closures, no trait objects. In-process
//! this rides a channel; across a process boundary it is line-delimited JSON, and
//! the shape is identical either way.
//!
//! ## One channel, not two — `INV-8`
//!
//! An adapter may run out-of-process, in which case it sees the event stream **and
//! nothing else** — a Rust `Result` is invisible to it. So every outcome the adapter
//! needs, including every failure, travels as an `Event`. Handlers use `Result`
//! internally and convert at the edge; nothing important is reported only by
//! returning it.
//!
//! See `design/invariants.md`.

use std::path::PathBuf;

use md2pdf_domain::Compromise;
use serde::{Deserialize, Serialize};

/// Something the user (or an adapter on their behalf) asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    /// Convert one Source into `destination`.
    ///
    /// No Template is named: the catalogue arrives in 3e, and until it exists a field
    /// for something nothing can supply would be speculative.
    ConvertSource {
        source: PathBuf,
        destination: PathBuf,
    },
}

/// Something that happened. The engine's only way of speaking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// The Source became Elements.
    ///
    /// **One event, not two.** The event storm lists `SourceParsed` and
    /// `MarkupEmitted` separately, and the domain does work in that order — but
    /// `convert()` parses and emits atomically, so the engine never observes the
    /// moment between them. Emitting both would claim a granularity the API does not
    /// have. The storm describes the domain; this describes what can actually be seen.
    ///
    /// `compromises` travels from here on (`INV-4`): a concession not emitted cannot
    /// be recovered later, even though nothing consumes it until the attention gate.
    SourceConverted {
        elements: usize,
        images: usize,
        compromises: Vec<Compromise>,
    },
    CompilationSucceeded {
        pages: usize,
    },
    CompilationFailed {
        message: String,
    },
    OutputWritten {
        path: PathBuf,
    },
    /// Anything else that stopped the Job — an unreadable Source, a refused overwrite.
    Failed {
        message: String,
    },
}

/// Where events go.
///
/// Deliberately a plain closure rather than a trait: an adapter passes
/// `|e| tx.send(e)`, a test passes `|e| collected.push(e)`, and nobody implements
/// anything. An earlier draft specified an `EventSink` trait; swapping a sink type
/// later is a mechanical refactor, so it failed the gate test in
/// `design/invariants.md` and the simple thing won.
pub type Emit<'a> = &'a mut dyn FnMut(Event);

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_domain::{CompromiseKind, ElementId};

    fn all_events() -> Vec<Event> {
        vec![
            Event::SourceConverted {
                elements: 12,
                images: 2,
                compromises: vec![Compromise {
                    id: ElementId::new(3, "#table(columns: 2, [a], [b])"),
                    kind: CompromiseKind::ImageMissing,
                    page: None,
                }],
            },
            Event::CompilationSucceeded { pages: 4 },
            Event::CompilationFailed {
                message: "unclosed delimiter".into(),
            },
            Event::OutputWritten {
                path: PathBuf::from("/out/notes.pdf"),
            },
            Event::Failed {
                message: "source unreadable".into(),
            },
        ]
    }

    /// `INV-8`: an out-of-process adapter sees only this. If a variant stops
    /// round-tripping, that boundary quietly closes.
    #[test]
    fn every_event_survives_a_json_round_trip() {
        for event in all_events() {
            let json = serde_json::to_string(&event).expect("serialise");
            let back: Event = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(back, event, "round-trip changed the event: {json}");
        }
    }

    #[test]
    fn every_command_survives_a_json_round_trip() {
        let command = Command::ConvertSource {
            source: PathBuf::from("/docs/notes.md"),
            destination: PathBuf::from("/out"),
        };
        let json = serde_json::to_string(&command).expect("serialise");
        let back: Command = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, command);
    }

    /// The wire format is a published surface. Renaming a variant is a deliberate
    /// break, so the tags are asserted rather than left to `rename_all`.
    #[test]
    fn the_wire_tags_are_what_we_think_they_are() {
        let json = serde_json::to_string(&Event::OutputWritten {
            path: PathBuf::from("/out/x.pdf"),
        })
        .unwrap();
        assert!(
            json.contains(r#""event":"output_written""#),
            "unexpected tag: {json}"
        );

        let json = serde_json::to_string(&Command::ConvertSource {
            source: PathBuf::from("a.md"),
            destination: PathBuf::from("out"),
        })
        .unwrap();
        assert!(
            json.contains(r#""command":"convert_source""#),
            "unexpected tag: {json}"
        );
    }

    #[test]
    fn a_closure_sink_collects_in_order() {
        let mut seen: Vec<Event> = Vec::new();
        {
            let mut emit = |e: Event| seen.push(e);
            let sink: Emit = &mut emit;
            for event in all_events() {
                sink(event);
            }
        }
        assert_eq!(seen.len(), 5);
        assert!(matches!(seen[0], Event::SourceConverted { .. }));
        assert!(matches!(seen[3], Event::OutputWritten { .. }));
    }

    /// A compromise that reaches an adapter must still name its Element (`INV-4`).
    #[test]
    fn compromises_survive_the_boundary_intact() {
        let event = all_events().remove(0);
        let json = serde_json::to_string(&event).unwrap();
        let Event::SourceConverted { compromises, .. } =
            serde_json::from_str::<Event>(&json).unwrap()
        else {
            panic!("wrong variant");
        };
        assert_eq!(compromises.len(), 1);
        assert_eq!(compromises[0].kind, CompromiseKind::ImageMissing);
        assert_eq!(compromises[0].id.order, 3);
    }
}
