//! The image seam, end to end (T12).
//!
//! convert → manifest → `Typesetter::add_file` → compiled PDF, with no filesystem:
//! the probe claims a path exists and the test supplies the bytes that `md2pdf-paths`
//! would have read. That is exactly the engine's job in 3c, stood in for here.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use md2pdf_convert::{convert, Conversion, ImageProbe, SourceContext};
use md2pdf_domain::{CompromiseKind, DecisionMap, Template};
use md2pdf_typeset::Typesetter;

const WIDE: &[u8] = include_bytes!("../../../md2pdf-typeset/tests/fixtures/wide-200x20.png");
const SQUARE: &[u8] = include_bytes!("../../../md2pdf-typeset/tests/fixtures/square-10x10.png");

struct Present(HashSet<PathBuf>);

impl Present {
    fn with(paths: &[&str]) -> Self {
        Self(paths.iter().map(PathBuf::from).collect())
    }
}

impl ImageProbe for Present {
    fn exists(&self, path: &Path) -> bool {
        self.0.contains(path)
    }
}

fn source_dir() -> PathBuf {
    PathBuf::from("/docs")
}

/// Stand in for the engine: read every manifest entry and register it.
fn register(ts: &Typesetter, conversion: &Conversion, bytes_for: &dyn Fn(&Path) -> &'static [u8]) {
    for (virtual_name, absolute) in &conversion.images {
        assert!(
            ts.add_file(virtual_name, bytes_for(absolute).to_vec()),
            "typeset refused the virtual name {virtual_name:?}"
        );
    }
}

#[test]
fn a_resolved_image_reaches_the_pdf() {
    let probe = Present::with(&["/docs/wide.png"]);
    let dir = source_dir();
    let c = convert("![a chart](wide.png)\n", &SourceContext::new(&dir, &probe));

    assert_eq!(c.images.len(), 1, "manifest should hold the one image");
    assert!(!c.is_flagged(), "a resolvable image is not a compromise");
    let (name, path) = c.images.iter().next().unwrap();
    assert_eq!(path, &PathBuf::from("/docs/wide.png"));
    assert!(
        c.elements[0]
            .body
            .as_str()
            .contains(&format!("image(\"{name}\")")),
        "markup does not reference the manifest name: {}",
        c.elements[0].body
    );

    let ts = Typesetter::new();
    register(&ts, &c, &|_| WIDE);
    let compilation = ts
        .render(
            &c.elements,
            &Template::default(),
            &DecisionMap { decisions: vec![] },
        )
        .expect("a registered image must compile");
    assert!(compilation.pdf().expect("pdf").starts_with(b"%PDF"));
}

/// The failure guard, as a property: every `#image("…")` emitted is a name the engine
/// will have registered. A name emitted without a manifest entry fails the **whole**
/// document, not one element.
#[test]
fn every_emitted_image_name_is_in_the_manifest() {
    let probe = Present::with(&["/docs/wide.png", "/docs/sub/square.png"]);
    let dir = source_dir();
    let md = "\
![one](wide.png)

Text with ![two](sub/square.png) inline.

![missing](gone.png)

![remote](https://x.test/a.png)

![bad](notes.txt)

![data](data:image/png;base64,AAAA)
";
    let c = convert(md, &SourceContext::new(&dir, &probe));

    for el in &c.elements {
        let body = el.body.as_str();
        let mut rest = body;
        while let Some(i) = rest.find("#image(\"") {
            rest = &rest[i + 8..];
            let end = rest.find('"').expect("unterminated image name");
            let name = &rest[..end];
            assert!(
                c.images.contains_key(name),
                "emitted #image({name:?}) with no manifest entry"
            );
            rest = &rest[end..];
        }
    }

    // And the whole thing must still compile, which is the point of the guard.
    let ts = Typesetter::new();
    register(&ts, &c, &|p| {
        if p.ends_with("square.png") {
            SQUARE
        } else {
            WIDE
        }
    });
    assert!(ts
        .render(
            &c.elements,
            &Template::default(),
            &DecisionMap { decisions: vec![] }
        )
        .is_ok());
}

#[test]
fn each_failure_mode_gets_its_own_compromise_kind() {
    let probe = Present::with(&[]);
    let dir = source_dir();

    let cases: [(&str, CompromiseKind); 2] = [
        ("![x](gone.png)", CompromiseKind::ImageMissing),
        ("![x](https://x.test/a.png)", CompromiseKind::ImageSkipped),
    ];
    for (md, expected) in cases {
        let c = convert(md, &SourceContext::new(&dir, &probe));
        assert!(
            c.compromises.iter().any(|x| x.kind == expected),
            "{md:?} did not record {expected:?}: {:?}",
            c.compromises
        );
        assert!(
            c.images.is_empty(),
            "{md:?} registered a file it cannot read"
        );
    }

    // Unsupported carries *why*, so the attention gate can explain itself.
    let c = convert("![x](notes.txt)", &SourceContext::new(&dir, &probe));
    assert!(c.compromises.iter().any(|x| matches!(
        &x.kind,
        CompromiseKind::UnsupportedConstruct { construct } if construct.contains("notes.txt")
    )));
}

#[test]
fn one_file_referenced_twice_is_registered_once() {
    // Dedup falls out of the naming scheme; without it a batch re-reads the same logo
    // for every mention.
    let probe = Present::with(&["/docs/logo.png"]);
    let dir = source_dir();
    let c = convert(
        "![a](logo.png)\n\n![b](./logo.png)\n",
        &SourceContext::new(&dir, &probe),
    );
    assert_eq!(
        c.images.len(),
        1,
        "same file registered twice: {:?}",
        c.images
    );
    assert_eq!(c.elements.len(), 2, "both mentions should still render");
}

#[test]
fn without_a_context_every_image_degrades_exactly_as_before() {
    // `SourceContext::none()` must reproduce Stage 1 behaviour precisely.
    let c = convert("![alt](wide.png)\n", &SourceContext::none());
    assert!(c.images.is_empty());
    assert!(c.is_flagged());
    assert!(!c.elements[0].body.as_str().contains("#image("));
}
