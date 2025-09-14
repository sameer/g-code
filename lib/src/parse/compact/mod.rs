//! Parsing for compact g-code representations

pub(crate) mod binary;
pub(crate) mod meatpack;

pub use binary::File as BinaryFile;
pub use meatpack::meatpacked_to_string;
