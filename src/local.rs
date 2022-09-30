use std::{cell::RefCell, collections::LinkedList, io, io::ErrorKind, net::SocketAddr, sync::Arc, time::Duration};

use futures::StreamExt;
use log::{debug, error, info, trace};
use tokio::{
    net::{lookup_host, TcpListener, TcpStream},
    time,
};
use tokio_kcp::KcpStream;
use tokio_yamux::{Config as YamuxConfig, Control as YamuxControl, Error as YamuxError, Session as YamuxSession};

use crate::{
    config::{Config, ServerAddr},
    opt::create_outbound_kcp,
};

/// Local mode
///
/// ```plain
///              TCP Loopback                KCP (UDP)
/// [SS-Client] <------------> [SSKCP-Local] --------> REMOTE
/// ```
pub async fn start_proxy(config: Config) -> io::Result<()> {
    debug!("start local proxy with {:?}", config);

    let config = Arc::new(config);

    let listener = match config.local_addr {
        ServerAddr::SocketAddr(sa) => TcpListener::bind(sa).await?,
        ServerAddr::DomainName(ref dname, port) => TcpListener::bind((dname.as_str(), port)).await?,
    };

    info!("KCP local listening on {}", listener.local_addr().unwrap());

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(err) => {
                error!("accept failed with error: {}", err);
                time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        debug!("accepted {}", peer_addr);

        let config = config.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_client(&config, stream, peer_addr).await {
                error!("failed to handle client {}, error: {}", peer_addr, err);
            }
        });
    }
}

async fn connect_server(config: &Config) -> io::Result<KcpStream> {
    let kcp_config = &config.kcp_config;

    match config.remote_addr {
        ServerAddr::SocketAddr(sa) => create_outbound_kcp(kcp_config, sa, &config.plugin_opts)
            .await
            .map_err(Into::into),
        ServerAddr::DomainName(ref dname, port) => {
            let mut result = None;

            for addr in lookup_host((dname.as_str(), port)).await? {
                match create_outbound_kcp(kcp_config, addr, &config.plugin_opts).await {
                    Ok(s) => {
                        result = Some(Ok(s));
                        break;
                    }
                    Err(err) => {
                        result = Some(Err(err));
                    }
                }
            }

            match result {
                None => Err(io::Error::new(ErrorKind::Other, "lookup_host resolved to empty")),
                Some(Ok(s)) => Ok(s),
                Some(Err(err)) => Err(err.into()),
            }
        }
    }
}

struct ConnectionPool {
    conns: LinkedList<YamuxControl>,
}

thread_local! {
    static CONNECTION_POOL: RefCell<ConnectionPool> = RefCell::new(ConnectionPool {
        conns: LinkedList::new()
    });
}

async fn handle_client(config: &Config, mut stream: TcpStream, _peer_addr: SocketAddr) -> io::Result<()> {
    // Take one valid connection
    let mut conn = loop {
        let yamux_conn = CONNECTION_POOL.with(|pool| {
            let pool = &mut pool.borrow_mut().conns;
            pool.pop_front()
        });

        if let Some(mut yamux_control) = yamux_conn {
            match yamux_control.open_stream().await {
                Ok(s) => {
                    trace!("yamux connection opened {:?}", s);

                    CONNECTION_POOL.with(|pool| {
                        pool.borrow_mut().conns.push_back(yamux_control);
                    });

                    break s;
                }
                Err(YamuxError::StreamsExhausted) => {
                    // Return it back to CONNECTION_POOL, then create a new connection
                    CONNECTION_POOL.with(|pool| {
                        pool.borrow_mut().conns.push_back(yamux_control);
                    });
                }
                Err(err) => {
                    error!("yamux connection open error: {}", err);
                    drop(yamux_control);
                }
            };
        }

        // Make a new connection
        let kcp_conn = match connect_server(config).await {
            Ok(c) => c,
            Err(err) => {
                error!("kcp server connect error, error: {}", err);
                continue;
            }
        };
        let mut yamux_session = YamuxSession::new_client(kcp_conn, YamuxConfig::default());
        let yamux_control = yamux_session.control();

        tokio::spawn(async move {
            loop {
                match yamux_session.next().await {
                    Some(Ok(..)) => {}
                    Some(Err(e)) => {
                        error!("yamux connection aborted with connection error: {}", e);
                        break;
                    }
                    None => {
                        trace!("yamux client session closed");
                        break;
                    }
                }
            }
        });

        CONNECTION_POOL.with(|pool| {
            let pool = &mut pool.borrow_mut().conns;
            pool.push_front(yamux_control);
        });

        trace!("kcp connection opened");
    };

    tokio::io::copy_bidirectional(&mut conn, &mut stream).await.map(|_| ())
}
