//! Blocking library preparation primitive shared by background and on-demand work.

use std::path::Path;

use super::CachedPcm;
use super::processing::ProcessingError;

/// Prepares one source file into the canonical cached PCM representation.
///
/// File I/O and decoding happen here; callers must run this function on a
/// blocking worker. Decoder repairs are represented in the returned analysis
/// metadata and cached PCM. Source files are never rewritten.
pub fn prepare(path: &Path) -> Result<CachedPcm, ProcessingError> {
    super::processing::decode_cached(path)
}
