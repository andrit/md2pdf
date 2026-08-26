//! Render a page and write it out as a PNG, so a human can look at it.
//!
//! `design/invariants.md` and the italic bug both say the same thing: a green test is
//! blind to how the page reads. This is the smallest thing that turns a rendered page
//! into something a person can open.
//!
//! Hand-rolled, and deliberately: an encoder needs `zlib`, which this crate does not
//! depend on and should not gain for a debugging tool. Deflate **stored** blocks are
//! uncompressed by definition, so the whole "compressor" is a length and its complement.
//!
//! ```text
//! LOOK_AT=documents/imgscalr/project-state.md LOOK_OUT=/tmp/x.png \
//!   cargo test -p md2pdf-engine --test walking_skeleton look_at_a_page \
//!     -- --ignored --nocapture
//! ```

/// CRC-32, as PNG chunks require.
fn crc32(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, e) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *e = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in bytes {
        c = table[((c ^ u32::from(*b)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_over = kind.to_vec();
    crc_over.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_over).to_be_bytes());
    out
}

/// RGBA pixels to a PNG, uncompressed.
pub fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    // Each scanline is prefixed with its filter type; 0 is "none".
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        let row = y * width as usize * 4;
        raw.extend_from_slice(&rgba[row..row + width as usize * 4]);
    }

    let mut z = vec![0x78, 0x01]; // zlib header, no preset dictionary
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = u8::from((i + 1) * 65535 >= raw.len());
        z.push(last);
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    out.extend_from_slice(&chunk(b"IHDR", &ihdr));
    out.extend_from_slice(&chunk(b"IDAT", &z));
    out.extend_from_slice(&chunk(b"IEND", &[]));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_convert::{convert, SourceContext};
    use md2pdf_domain::Template;
    use md2pdf_typeset::Typesetter;

    #[test]
    #[ignore = "debugging tool, not a gate"]
    fn look_at_a_page() {
        let src = std::env::var("LOOK_AT").expect("LOOK_AT");
        let out = std::env::var("LOOK_OUT").expect("LOOK_OUT");
        let page: usize = std::env::var("LOOK_PAGE")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);

        // Through the broker, not `std::fs`: INV-9 holds for debugging tools too, and
        // check-boundaries.sh caught this file the first time it was written.
        let broker = md2pdf_paths::PathBroker::new();
        // `LOOK_TEMPLATE=<path to template.toml>` renders through a real template rather
        // than the built-in defaults — which is how a template change gets *looked at*
        // rather than asserted (3e exit criterion 1).
        let template = match std::env::var("LOOK_TEMPLATE") {
            Ok(path) => md2pdf_template::TemplateFile::parse(
                &broker
                    .read_to_string(std::path::Path::new(&path))
                    .expect("template"),
            )
            .expect("template parses")
            .to_template(),
            Err(_) => Template::default(),
        };
        let markdown = broker
            .read_to_string(std::path::Path::new(&src))
            .expect("source");
        let parent = std::path::Path::new(&src).parent().unwrap().to_path_buf();
        let images = super::super::census::CorpusImages;
        let conversion = convert(&markdown, &SourceContext::new(&parent, &images));

        let ts = Typesetter::new();
        let (d, _) = ts.probe(&conversion.elements, &template).expect("probe");
        let c = ts
            .render(&conversion.elements, &template, &d)
            .expect("render");
        let (w, h, rgba) = c.raster(page, 2.0).expect("raster");
        broker
            .overwrite(std::path::Path::new(&out), &png(w, h, &rgba))
            .expect("write");
        println!("wrote {out} ({w}x{h}, page {page} of {})", c.page_count());
    }

    /// Draw the application icon, with the application.
    ///
    /// md2pdf already turns text into pixels — using anything else to make its icon would
    /// mean a drawing dependency for one 1024x1024 square. The wordmark is set in the same
    /// Source Sans 3 the documents are, so the icon is literally a page md2pdf rendered.
    ///
    /// **`ICON_OUT` must be absolute.** `cargo test` runs with the working directory set
    /// to the *crate* root, so a relative path writes into `crates/md2pdf-engine/` rather
    /// than the repository's `assets/` — quietly, and it looks like it worked.
    ///
    /// ```text
    /// ICON_OUT=$PWD/assets/icon/md2pdf-1024.png cargo test -p md2pdf-engine \
    ///     --test walking_skeleton draw_the_icon -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "asset generation, run deliberately"]
    fn draw_the_icon() {
        use md2pdf_domain::{Element, ElementClass, Markup};

        let out = std::env::var("ICON_OUT").expect("ICON_OUT");
        let broker = md2pdf_paths::PathBroker::new();

        // A square page, and the raster scale that turns 256pt into 1024px.
        let template = Template {
            page_width_pt: 256.0,
            page_height_pt: 256.0,
            margin_pt: 0.0,
            ..Template::default()
        };

        // Set directly rather than converted from markdown: this is a wordmark, not a
        // document, and markdown has no way to say "two lines, centred, very large".
        // `r##"..."##`: the wordmark contains `"#` inside `rgb("#1b1b1f")`, which would
        // close a `r#"..."#` string early.
        // A filled rect rather than `#set page(fill:)`: the ProbePass wraps every element
        // in a container, and Typst refuses page configuration inside one. The rect is
        // sized in points to match the page exactly.
        let body = r##"#rect(
  width: 256pt, height: 256pt, fill: rgb("#1b1b1f"), stroke: none, radius: 56pt,
)[
  #set text(font: "Source Sans 3", fill: rgb("#f5f5f7"), weight: "bold")
  #align(center + horizon)[
    #block(spacing: 4pt)[
      #text(size: 76pt, tracking: -3pt)[MD2]#linebreak()#text(size: 76pt, tracking: -3pt)[PDF]
    ]
  ]
]"##;
        let el = Element::new(0, ElementClass::Prose, Markup::raw(body.to_string()));

        let ts = Typesetter::new();
        let (d, _) = ts
            .probe(std::slice::from_ref(&el), &template)
            .expect("probe");
        let c = ts
            .render(std::slice::from_ref(&el), &template, &d)
            .expect("render");
        let (w, h, rgba) = c.raster(0, 4.0).expect("raster");
        broker
            .overwrite(std::path::Path::new(&out), &png(w, h, &rgba))
            .expect("write");
        println!("wrote {out} ({w}x{h})");
    }
}
