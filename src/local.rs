use std::io;

use futures::{self, Future, Stream};
use futures::future::Either;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use tokio_kcp::KcpStream;

use config::Config;
use dns_resolver::resolve_server_addr;
use protocol::{copy_decode, copy_encode};

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
                let conv = ::current();
                match kcp_config {
                    Some(ref c) => KcpStream::connect_with_config(conv, &svr_addr, &chandle, c),
                    None => KcpStream::connect(conv, &svr_addr, &chandle),
                }
            });

            stream.and_then(move |remote| {
                let (cr, cw) = client.split();
                let (rr, rw) = remote.split();
                copy_encode(cr, rw).select2(copy_decode(rr, cw))
                    .then(move |r| {
                        match r {
                            Ok(Either::A((n, o))) => {
                                debug!("Connection {} is closed, relayed {}bytes", addr, n);
                                // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                                Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                            }
                            Ok(Either::B((n, o))) => {
                                debug!("Connection {} is closed, relayed {}bytes", addr, n);
                                // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                                Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                            }
                            Err(Either::A((err, o))) => {
                                error!("Connection {} is closed with error {}", addr, err);
                                Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                                // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                            }
                            Err(Either::B((err, o))) => {
                                error!("Connection {} is closed with error {}", addr, err);
                                // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                                Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                            }
                        }
                    })
                    .map(|_| ())
                // copy_encode(cr, rw).join(copy_decode(rr, cw)).map(|_| ())
            })
        });

        handle.spawn(fut.map_err(move |err| {
                                     error!("Relay error, addr: {}, err: {:?}", addr, err);
                                 }));

        Ok(())
    });

    core.run(svr)
}
