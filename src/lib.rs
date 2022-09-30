//! KCP proxy for ShadowSocks

pub mod config;
pub mod local;
pub mod opt;
pub mod server;
mod sys;

pub use self::sys::adjust_nofile;
