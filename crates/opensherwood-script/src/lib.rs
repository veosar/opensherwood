//! Script execution for OpenSherwood.
//!
//! The `.scb` instruction set is not specified yet (`docs/formats/scb.md`); this crate currently only
//! re-exports the header parser so the dependency direction is established.

pub use opensherwood_formats::scb::{ScbHeader, parse_header};
