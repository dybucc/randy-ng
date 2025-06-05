//! This crate contains all the main code for the game randy-ng, a new generation randy with an
//! actual TUI framework.

#![expect(
    unused_crate_dependencies,
    reason = "The dependencies are used in the library crate of the same name."
)]

use clap::Parser as _;
use color_eyre::{eyre::eyre, install, Result};
use randy_ng::{App, Cli};
use ratatui::{init, restore};
use ureq::Error;

fn main() -> Result<()> {
    install()?;
    let _ = Cli::parse();

    let terminal = init();
    let result = App::default().run(terminal);
    restore();

    match result.err() {
        None => Ok(()),
        Some(err) => match err.downcast::<Error>() {
            Ok(err) => match err {
                Error::StatusCode(status) => match status {
                    400 => Err(eyre!("bad request")),
                    401 => Err(eyre!("invalid credentials")),
                    402 => Err(eyre!("insufficient credits")),
                    403 => Err(eyre!("flagged input")),
                    408 => Err(eyre!("timed out")),
                    429 => Err(eyre!("rate limited")),
                    502 => Err(eyre!("invalid response or model down")),
                    503 => Err(eyre!("no available providers")),
                    _ => Err(eyre!("unknown error")),
                },
                _ => Err(eyre!("unknown error")),
            },
            Err(_) => Err(eyre!("unknown error")),
        },
    }
}
