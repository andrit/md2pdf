//! Collision, Resolution, BlanketResolution. See `design/GLOSSARY.md`.
//!
//! Unlike `SourceSet` and `ImageManifest` — which live in the crate that produces them
//! because nothing below needs them — these types belong in the domain: they cross the
//! engine's Command boundary, so **an adapter has to be able to construct one**, and
//! an adapter depends on the domain.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// An OutputPath that already exists on disk.
///
/// Carries the Source as well as the path, because the user is being asked about a
/// *conversion*, not about a file: "notes.md would overwrite notes.pdf" is answerable,
/// "notes.pdf exists" is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collision {
    pub source: PathBuf,
    pub output: PathBuf,
}

/// The user's answer to one Collision.
///
/// **There is no silent overwrite anywhere in the app** (`INV-3`). `Overwrite` is a
/// recorded human decision, which is exactly why it is a value that has to be supplied
/// rather than a default that can be fallen into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Write beside the existing file under a suffixed name.
    Rename,
    /// Convert nothing for this Source; leave the existing file alone.
    Skip,
    /// Replace the existing file. Only ever because someone said so.
    Overwrite,
}

/// A Resolution applied to every Collision in a batch without prompting again.
///
/// Without this, a batch of fifty with a prompt per Collision is the worst part of the
/// app. It is a distinct type from [`Resolution`] rather than an alias because the two
/// are answers to different questions — "what about this one?" versus "what about all
/// of them?" — and a future per-Collision prompt will need both at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlanketResolution {
    RenameAll,
    SkipAll,
    OverwriteAll,
}

impl BlanketResolution {
    /// The per-Collision answer this blanket choice implies.
    pub fn resolution(self) -> Resolution {
        match self {
            Self::RenameAll => Resolution::Rename,
            Self::SkipAll => Resolution::Skip,
            Self::OverwriteAll => Resolution::Overwrite,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blanket_answer_maps_to_the_single_one() {
        assert_eq!(
            BlanketResolution::RenameAll.resolution(),
            Resolution::Rename
        );
        assert_eq!(BlanketResolution::SkipAll.resolution(), Resolution::Skip);
        assert_eq!(
            BlanketResolution::OverwriteAll.resolution(),
            Resolution::Overwrite
        );
    }

    #[test]
    fn resolutions_cross_the_wire() {
        // They ride on a Command, so an out-of-process adapter must be able to send one.
        for r in [
            BlanketResolution::RenameAll,
            BlanketResolution::SkipAll,
            BlanketResolution::OverwriteAll,
        ] {
            let json = serde_json::to_string(&r).expect("serialise");
            assert_eq!(
                serde_json::from_str::<BlanketResolution>(&json).expect("deserialise"),
                r
            );
        }
    }
}
