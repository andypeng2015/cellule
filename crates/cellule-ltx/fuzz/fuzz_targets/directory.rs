#![no_main]

//! Directory nodes must reject malformed headers, records, and aggregates.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = cellule_ltx::internal::inspect_directory_node(data);
});
