//! Asynchronous DNS resolver

use std::cell::RefCell;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use futures::{BoxFuture, Future};
use futures::future;
use lru_time_cache::LruCache;
use netdb::hosts::poll_host_by_name;
use tokio_core::reactor::Handle;

use config::ServerAddr;

// Global DNS cache.
// Cache for 5mins and store at most 128 entries
thread_local!(static GLOBAL_DNS_CACHE: RefCell<LruCache<String, IpAddr>>
    = RefCell::new(LruCache::with_expiry_duration_and_capacity(Duration::from_secs(5 * 60), 128)));

pub fn resolve(addr: &str, handle: &Handle) -> BoxFuture<IpAddr, io::Error> {
    // FIXME: Sometimes addr is actually an IpAddr!
    if let Ok(addr) = addr.parse::<IpAddr>() {
        return future::finished(addr).boxed();
    }

    let cached_addr = GLOBAL_DNS_CACHE.with(|c| {
                                                let mut xc = c.borrow_mut();
                                                xc.get(addr).map(|x| *x)
                                            });

    if let Some(addr) = cached_addr {
        return future::finished(addr).boxed();
    }

    let owned_addr = addr.to_owned();
    trace!("Going to resolve \"{}\"", owned_addr);
    poll_host_by_name(addr, handle)
        .and_then(move |hn| match hn {
                      None => {
                          let err = io::Error::new(io::ErrorKind::Other,
                                                   format!("Failed to resolve \"{}\"", owned_addr));
                          Err(err)
                      }
                      Some(hn) => {
                          let addrs = hn.addrs();
                          trace!("Resolved \"{}\" as {:?}", owned_addr, addrs);

                          if addrs.is_empty() {
                              let err = io::Error::new(io::ErrorKind::Other,
                                                       format!("Failed to resolve \"{}\"", owned_addr));
                              Err(err)
                          } else {
                              let addr = addrs[0];
                              trace!("Resolved \"{}\" as {}", owned_addr, addr);
                              GLOBAL_DNS_CACHE.with(|c| {
                                                        let mut xc = c.borrow_mut();
                                                        xc.insert(owned_addr, addr);
                                                    });
                              Ok(addr)
                          }
                      }
                  })
        .boxed()
}

pub fn resolve_server_addr(addr: &ServerAddr, handle: &Handle) -> BoxFuture<SocketAddr, io::Error> {
    match *addr {
        ServerAddr::SocketAddr(ref s) => future::finished(*s).boxed(),
        ServerAddr::DomainName(ref domain, port) => {
            resolve(domain, handle)
                .map(move |ip| SocketAddr::new(ip, port))
                .boxed()
        }
    }
}
