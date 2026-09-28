#![no_main]

//! Root documents and descriptor pages must reject malformed JSON and extents.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = cellule_ltx::internal::inspect_root(data);
    let _ = cellule_ltx::internal::inspect_segment_page(data);
});
