//! Shared types for smarty-pants daemon and CLI.
//!
//! No I/O lives here — only the data definitions that cross the socket
//! and the few path/config helpers both binaries need.

pub mod paths;
pub mod protocol;
pub mod config;
