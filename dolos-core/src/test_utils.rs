//! Helpers shared by the unit tests of this crate.

use crate::Symbol;

/// One symbol per byte of `s`.
pub fn str_to_symbols(s: &str) -> Vec<Symbol> {
    s.as_bytes().iter().map(|&b| b as Symbol).collect()
}
