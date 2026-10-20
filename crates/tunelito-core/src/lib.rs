//! Shared Tunelito logic: keys, the on/off state machine, sshd plans, and the helper hello.
//!
//! Operating-system service control (installing OpenSSH, creating the dedicated user,
//! starting cloudflared) is planned here and executed by the host app. Windows and macOS
//! command execution is not run in CI. Linux CI executes the file layout, parsers, state
//! machine, and helper CLI, and checks Linux command arguments without applying them to
//! the machine.

#![forbid(unsafe_code)]

pub mod access;
pub mod checksum;
pub mod cloudflared;
pub mod error;
pub mod hello;
pub mod host;
pub mod keys;
pub mod locales;
pub mod logs;
pub mod netinfo;
pub mod paths;
pub mod plan;
pub mod sessions;
pub mod ssh_config;
pub mod sshd;
pub mod state;

pub use error::Error;
pub use host::{Effect, Host, Settings, ViewModel};
pub use keys::DEDICATED_USER;
