use std::io;

use futures::{Future, Stream};
// use futures::future::Either;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use tokio_io::io::{read_exact, write_all, flush};
use tokio_kcp::KcpListener;

use config::Config;
use dns_resolver::resolve_server_addr;
use protocol::{copy_decode, copy_encode};

/// Local mode
///
/// ```plain
///        KCP (UDP)                 TCP Loopback
/// CLIENT ---------> [SSKCP-Server] <----------> [SS-Server]
/// ```
pub fn start_proxy(config: &Config) -> io::Result<()> {
    debug!("Start server proxy with {:?}", config);

    let mut core = Core::new()?;
    let handle = core.handle();

    let svr_addr = config.remote_addr.listen_addr();
    let listener = match config.kcp_config {
        Some(c) => KcpListener::bind_with_config(svr_addr, &handle, c)?,
        None => KcpListener::bind(svr_addr, &handle)?,
    };

    info!("Listening on {}", svr_addr);

    let svr = listener.incoming().for_each(|(client, addr)| {
        debug!("Accepted KCP connection {}, relay to {}", addr, config.local_addr);
        let chandle = handle.clone();
        let fut = resolve_server_addr(&config.local_addr, &handle).and_then(move |svr_addr| {
            read_exact(client, [0u8; 9])
                .and_then(|(client, buf)| write_all(client, buf))
                .and_then(|(client, _)| flush(client))
                .and_then(move |client| {
                    let stream = TcpStream::connect(&svr_addr, &chandle);
                    stream.and_then(move |remote| {
                        let (cr, cw) = client.split();
                        let (rr, rw) = remote.split();
                        // copy_decode(cr, rw).select2(copy_encode(rr, cw))
                        //     .then(move |r| {
                        //         match r {
                        //             Ok(Either::A((n, _o))) => {
                        //                 debug!("Connection {} is closed, relayed {}bytes", addr, n);
                        //                 // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                        //                 // Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                        //                 Ok(())
                        //             }
                        //             Ok(Either::B((n, _o))) => {
                        //                 debug!("Connection {} is closed, relayed {}bytes", addr, n);
                        //                 // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                        //                 // Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                        //                 Ok(())
                        //             }
                        //             Err(Either::A((err, _o))) => {
                        //                 error!("Connection {} is closed with error {}", addr, err);
                        //                 // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                        //                 // Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                        //                 Err(err)
                        //             }
                        //             Err(Either::B((err, _o))) => {
                        //                 error!("Connection {} is closed with error {}", addr, err);
                        //                 // Box::new(o.close()) as Box<Future<Item=u64, Error=io::Error>>
                        //                 // Box::new(o) as Box<Future<Item=u64, Error=io::Error>>
                        //                 Err(err)
                        //             }
                        //         }
                        //     })
                        //     // .map(|_| ())
                        copy_decode(cr, rw).join(copy_encode(rr, cw)).map(|_| ())
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
