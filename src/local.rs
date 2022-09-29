use std::{cell::RefCell, collections::LinkedList, io, io::ErrorKind, net::SocketAddr, sync::Arc, time::Duration};

use log::{debug, error, info, trace};
use once_cell::sync::Lazy;
use tokio::{
    net::{lookup_host, TcpListener, TcpStream},
    time,
};
use tokio_kcp::{KcpConfig, KcpStream};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use yamux::{
    Config as YamuxConfig,
    Connection as YamuxConnection,
    ConnectionError as YamuxConnectionError,
    Control as YamuxControl,
    Mode as YamuxMode,
};

use crate::config::{Config, ServerAddr};

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
    static DEFAULT_KCP_CONFIG: Lazy<KcpConfig> = Lazy::new(|| KcpConfig::default());

    let kcp_config = match config.kcp_config {
        Some(ref c) => c,
        None => &DEFAULT_KCP_CONFIG,
    };

    match config.remote_addr {
        ServerAddr::SocketAddr(sa) => KcpStream::connect(kcp_config, sa).await.map_err(Into::into),
        ServerAddr::DomainName(ref dname, port) => {
            let mut result = None;

            for addr in lookup_host((dname.as_str(), port)).await? {
                match KcpStream::connect(kcp_config, addr).await {
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
    let conn = loop {
        let yamux_conn = CONNECTION_POOL.with(|pool| {
            let pool = &mut pool.borrow_mut().conns;
            pool.pop_front()
        });

        if let Some(mut yamux_control) = yamux_conn {
            match yamux_control.open_stream().await {
                Ok(s) => {
                    trace!("yamux connection opened {:?}", s);
                    break s;
                }
                Err(YamuxConnectionError::TooManyStreams) => {
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
        let kcp_conn = connect_server(config).await?;
        let mut yamux_conn = YamuxConnection::new(kcp_conn.compat(), YamuxConfig::default(), YamuxMode::Client);
        let yamux_control = yamux_conn.control();

        tokio::spawn(async move {
            loop {
                match yamux_conn.next_stream().await {
                    Ok(Some(_)) => continue,
                    Ok(None) => break,
                    Err(e) => {
                        error!("yamux connection aborted with connection error: {}", e);
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

    let mut conn = conn.compat();
    tokio::io::copy_bidirectional(&mut conn, &mut stream).await.map(|_| ())
}
