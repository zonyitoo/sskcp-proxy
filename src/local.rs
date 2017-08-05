use std::io;

use futures::{self, Future, Stream};
use futures::future::Either;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use tokio_io::io::copy;
use tokio_kcp::KcpStream;

use config::Config;
use dns_resolver::resolve_server_addr;

/// Local mode
///
/// ```plain
///              TCP Loopback                KCP (UDP)
/// [SS-Client] <------------> [SSKCP-Local] --------> REMOTE
/// ```
pub fn start_proxy(config: &Config) -> io::Result<()> {
    debug!("Start local proxy with {:?}", config);

    let mut core = Core::new()?;
    let handle = core.handle();

    let svr_addr = config.local_addr.listen_addr();
    let listener = TcpListener::bind(svr_addr, &handle)?;

    info!("Listening on {}", svr_addr);

    let svr = listener.incoming().for_each(|(client, addr)| {
        debug!("Accepted TCP connection {}, relay to {}", addr, &config.remote_addr);
        let chandle = handle.clone();
        let kcp_config = config.kcp_config;
        let fut = resolve_server_addr(&config.remote_addr, &handle).and_then(move |svr_addr| {
            let stream = futures::lazy(move || {
                match kcp_config {
                    Some(ref c) => KcpStream::connect_with_config(&svr_addr, &chandle, c),
                    None => KcpStream::connect(&svr_addr, &chandle),
                }
            });

            stream.and_then(move |remote| {
                let (cr, cw) = client.split();
                let (rr, rw) = remote.split();
                copy(cr, rw).select2(copy(rr, cw))
                    .then(move |r| {
                        match r {
                            Ok(..) => {
                                debug!("Connection {} is closed", addr);
                                Ok(())
                            }
                            Err(Either::A((err, ..))) => {
                                error!("Connection {} is closed with error {}", addr, err);
                                Err(err)
                            }
                            Err(Either::B((err, ..))) => {
                                error!("Connection {} is closed with error {}", addr, err);
                                Err(err)
                            }
                        }
                    })
            })
        });

        handle.spawn(fut.map_err(move |err| {
                                     error!("Relay error, addr: {}, err: {:?}", addr, err);
                                 }));

        Ok(())
    });

    core.run(svr)
}
