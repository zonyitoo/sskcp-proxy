//! KCP proxy for ShadowSocks

extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_kcp;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_urlencoded;
extern crate futures;
extern crate subprocess;
#[macro_use]
extern crate log;
extern crate netdb;
extern crate lru_time_cache;

pub mod local;
pub mod server;
pub mod opt;
mod dns_resolver;
pub mod plugin;
pub mod config;
