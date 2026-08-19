//! One small, stable, dependency-free hash, shared rather than duplicated.
//!
//! Used for two things that both need *stability* rather than security: the
//! `ElementId` content hash, which detects that a Source was edited under a persisted
//! Override, and image virtual names, which must be identical across runs or `comemo`
//! memoisation misses on every recompile.

/// FNV-1a, 64-bit.
///
/// **Not cryptographic.** It detects change; it does not resist an adversary. Both
/// callers want a short, deterministic, dependency-free digest, and neither is
/// defending against anything.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic() {
        assert_eq!(fnv1a(b"/a/b/c.png"), fnv1a(b"/a/b/c.png"));
    }

    #[test]
    fn distinguishes_similar_inputs() {
        assert_ne!(fnv1a(b"/a/b/c.png"), fnv1a(b"/a/b/d.png"));
        assert_ne!(fnv1a(b"a"), fnv1a(b"A"));
    }

    #[test]
    fn matches_the_reference_vector() {
        // FNV-1a 64 of "" is the offset basis; of "a" the documented value.
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
