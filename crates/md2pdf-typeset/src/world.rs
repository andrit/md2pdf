//! The `World` implementation. Confined here with everything else typst-shaped.

use std::cell::RefCell;
use std::collections::HashMap;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::fonts::FontLibrary;

/// `None` when the name is not a usable virtual path.
pub(crate) fn try_file_id(name: &str) -> Option<FileId> {
    VirtualPath::new(name)
        .ok()
        .map(|vpath| RootedPath::new(VirtualRoot::Project, vpath).intern())
}

pub(crate) fn file_id(name: &str) -> FileId {
    try_file_id(name).expect("valid vpath")
}

/// Long-lived so that `comemo` memoisation survives between compilations — that is
/// what makes an Override cost ~8ms instead of a cold compile.
pub struct TypstWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main_id: FileId,
    main: RefCell<Source>,
    /// Virtual files the document may reference — images, in practice.
    ///
    /// Typst cannot load a file the `World` will not serve, and an unresolvable file
    /// is a compilation error for the **whole document**, not a skipped element. So
    /// everything a document references must be in here before compiling.
    ///
    /// Safe to mutate between compilations despite the long-lived `World`: `comemo`
    /// tracks `World::file` accesses and invalidates correctly. Verified — replacing
    /// bytes under the same name changes the next measurement (see
    /// `replacing_bytes_is_not_served_stale` in `tests/contract.rs`).
    files: RefCell<HashMap<FileId, Bytes>>,
}

// Compilation is driven from one thread at a time; the RefCell never escapes.
unsafe impl Send for TypstWorld {}
unsafe impl Sync for TypstWorld {}

impl TypstWorld {
    pub fn new() -> Self {
        let lib = FontLibrary::shipped();
        let main_id = file_id("main.typ");
        Self {
            library: LazyHash::new(Library::builder().build()),
            book: LazyHash::new(lib.book),
            fonts: lib.fonts,
            main_id,
            main: RefCell::new(Source::new(main_id, String::new())),
            files: RefCell::new(HashMap::new()),
        }
    }

    /// Register bytes under a virtual name, replacing any previous entry.
    ///
    /// Returns `false` when the name is not a usable virtual path — a library must not
    /// panic on a caller's string, and the name ultimately derives from a filesystem
    /// path this crate never sees.
    pub fn add_file(&self, name: &str, bytes: Vec<u8>) -> bool {
        match try_file_id(name) {
            Some(id) => {
                self.files.borrow_mut().insert(id, Bytes::new(bytes));
                true
            }
            None => false,
        }
    }

    /// Drop every registered file. Call **between Jobs, not between Sources.**
    ///
    /// The reason is memory, not correctness. Sharing files across the documents of one
    /// batch is *desirable*: virtual names are derived from the resolved absolute path,
    /// so two Sources referencing the same logo produce the same name for the same
    /// bytes. Clearing per Source would re-register that logo for every document in the
    /// batch and throw away the `comemo` hit this long-lived `World` exists to keep.
    pub fn clear_files(&self) {
        self.files.borrow_mut().clear();
    }

    pub fn set_source(&self, text: String) {
        *self.main.borrow_mut() = Source::new(self.main_id, text);
    }
}

impl Default for TypstWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl World for TypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main_id
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main.borrow().clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_without_slash().into()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .borrow()
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
