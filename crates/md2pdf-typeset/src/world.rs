//! The `World` implementation. Confined here with everything else typst-shaped.

use std::cell::RefCell;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::fonts::FontLibrary;

pub(crate) fn file_id(name: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(name).expect("valid vpath"),
    )
    .intern()
}

/// Long-lived so that `comemo` memoisation survives between compilations — that is
/// what makes an Override cost ~8ms instead of a cold compile.
pub struct TypstWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main_id: FileId,
    main: RefCell<Source>,
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
        }
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
        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
