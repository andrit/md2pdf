//! The macOS Finder panel, and nothing else.
//!
//! ## Why this is its own crate
//!
//! **So the gate can typecheck it.** `md2pdf-gui` cannot be checked for macOS from the
//! container at all — typst's `psm` dependency has a C build script that needs a macOS C
//! compiler, and `cargo check --target aarch64-apple-darwin` dies there before it reaches
//! any of our code. This crate's whole dependency graph is `objc2` and two of its
//! generated framework bindings, none of which compile any C, so
//! `cargo check --target aarch64-apple-darwin -p md2pdf-mac` works on Linux and is a step
//! in `verify.sh`.
//!
//! That is the same argument that put the state machine in `md2pdf-app`, applied to the
//! last unverifiable surface: make the part nothing can check as small as possible, then
//! find a way to check it anyway.
//!
//! **What is still not verified here**: that the panel behaves. Typechecking proves the
//! selectors exist with the shapes we call them with; it cannot prove the panel appears,
//! or that a person can dismiss it. That needs the Mac.
//!
//! ## Why not `rfd`
//!
//! `rfd` is a wrapper over exactly these calls. It is not in the vendored tree, so taking
//! it would mean a vendoring round for ~40 lines of binding — `plan-app.md` declined that
//! trade in part 3 and it is still the right call now that the bindings turn out to be
//! sitting in `vendor/` already.

#![cfg_attr(not(target_os = "macos"), allow(unused))]

use std::path::{Path, PathBuf};

/// Ask the user to pick a folder, starting at `start` if it is still there.
///
/// `None` means they cancelled — **or that there is no panel to show**, which is every
/// non-macOS build and any call from a thread that is not the main one. A caller treats
/// all of those the same way: nothing was chosen, so nothing changes, and the text field
/// is still there to type into.
///
/// **Blocks until dismissed.** `runModal` runs its own event loop, so the window stops
/// drawing while the panel is up. That is what every Mac application does and what makes
/// the result usable as a plain return value instead of another channel.
#[cfg(target_os = "macos")]
pub fn choose_folder(start: Option<&Path>) -> Option<PathBuf> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{NSString, NSURL};

    // `None` off the main thread rather than a panic: AppKit is main-thread-only, and a
    // file dialog is not worth taking the process down over. In this app it is always
    // called from `eframe`'s update loop, which *is* the main thread.
    let mtm = MainThreadMarker::new()?;
    let panel = NSOpenPanel::openPanel(mtm);

    // A folder chooser, not a file chooser: the destination is a directory, and letting
    // someone pick `report.pdf` here would mean guessing whether they meant its parent.
    panel.setCanChooseDirectories(true);
    panel.setCanChooseFiles(false);
    panel.setAllowsMultipleSelection(false);
    // The one affordance a text field cannot offer: making the folder you meant.
    panel.setCanCreateDirectories(true);
    panel.setPrompt(Some(&NSString::from_str("Save PDFs Here")));

    // Open where they already are. Skipped when the path does not exist, because a panel
    // that opens on nothing is more confusing than one that opens at the default.
    if let Some(dir) = start.filter(|d| d.is_dir()) {
        let path = NSString::from_str(&dir.to_string_lossy());
        let url = NSURL::fileURLWithPath(&path);
        panel.setDirectoryURL(Some(&url));
    }

    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    // `URLs` is plural because the panel can be a multi-selector; this one is not, so
    // there is exactly one or none.
    let url = panel.URLs().firstObject()?;
    let path = url.path()?;
    Some(PathBuf::from(path.to_string()))
}

/// The same call on every other platform: there is no panel, so nothing is chosen.
///
/// Present rather than `#[cfg]`-ed away at the call site, so `md2pdf-gui` stays one
/// program rather than two — and so a Linux build of the window still compiles.
#[cfg(not(target_os = "macos"))]
pub fn choose_folder(_start: Option<&Path>) -> Option<PathBuf> {
    None
}

/// Whether a folder panel can actually be shown, so the window can leave the button out
/// rather than drawing one that never does anything.
pub const fn can_choose() -> bool {
    cfg!(target_os = "macos")
}
