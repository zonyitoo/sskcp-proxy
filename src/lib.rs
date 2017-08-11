//! KCP proxy for ShadowSocks

#[macro_use]
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_kcp;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_urlencoded;
#[macro_use]
extern crate futures;
extern crate subprocess;
#[macro_use]
extern crate log;
extern crate netdb;
extern crate lru_time_cache;
extern crate bytes;
extern crate time;

use time::Timespec;

pub mod local;
pub mod server;
pub mod opt;
mod dns_resolver;
pub mod plugin;
pub mod config;
mod protocol;

#[inline]
fn as_millisec(timespec: &Timespec) -> u32 {
    (timespec.sec * 1000 + timespec.nsec as i64 / 1000 / 1000) as u32
}

#[inline]
fn current() -> u32 {
    let timespec = time::get_time();
    as_millisec(&timespec)
}
