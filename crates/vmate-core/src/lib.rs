//! vmate-core — library of domain logic for vmate.
//!
//! This crate is deliberately UI- and CLI-agnostic. It owns the "what":
//! country filtering, SQLite storage, OpenVPN process management, scan and
//! connect orchestration, geo lookup and export. The `vmate-cli` crate owns
//! the "how it looks": clap parsing, progress bars, TUIs and clipboard.
//!
//! External effects (running OpenVPN, killing processes, geo lookup) are
//! abstracted behind traits so that the behaviour can be unit tested with
//! fakes and replaced in production.

pub mod connect;
pub mod country;
pub mod db;
pub mod error;
pub mod export;
pub mod filter;
pub mod geo;
pub mod hash;
pub mod ovpn;
pub mod paths;
pub mod scan;
pub mod system;

pub use country::{CountryCode, CountryError};
pub use error::ParseError;
pub use filter::CountryFilter;
