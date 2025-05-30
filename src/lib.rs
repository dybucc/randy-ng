//! This crate contains all the functionality of the binary crate of the same name.

#![expect(
    clippy::cargo_common_metadata,
    reason = "Temporary during development prior to crates.io publishing."
)]
#![expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Temporary allow during development."
)]

mod app;
mod ui;
mod utils;

pub use app::App;
pub use utils::Cli;
