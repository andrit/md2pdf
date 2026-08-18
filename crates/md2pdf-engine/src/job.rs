//! Job orchestration: convert -> probe -> harvest -> render -> write.
//!
//! The imperative shell for one conversion. The decisions it applies are made by
//! the pure core; this module sequences the passes and owns the I/O.
//!
//! Not yet implemented.
