use std::{io, io::ErrorKind, net::SocketAddr, sync::Arc, time::Duration};

use log::{debug, error, info};
use once_cell::sync::Lazy;
use tokio::{
    net::{lookup_host, TcpListener, TcpStream},
    time,
};
use tokio_kcp::{KcpConfig, KcpStream};

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

async fn handle_client(config: &Config, mut stream: TcpStream, _peer_addr: SocketAddr) -> io::Result<()> {
    static DEFAULT_KCP_CONFIG: Lazy<KcpConfig> = Lazy::new(|| KcpConfig::default());

    let kcp_config = match config.kcp_config {
        Some(ref c) => c,
        None => &DEFAULT_KCP_CONFIG,
    };

    let mut remote_stream = match config.remote_addr {
        ServerAddr::SocketAddr(sa) => KcpStream::connect(kcp_config, sa).await?,
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
                None => return Err(io::Error::new(ErrorKind::Other, "lookup_host resolved to empty")),
                Some(Ok(s)) => s,
                Some(Err(err)) => return Err(err.into()),
            }
        }
    };

    tokio::io::copy_bidirectional(&mut remote_stream, &mut stream)
        .await
        .map(|_| ())
}
