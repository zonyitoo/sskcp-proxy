use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use log::{debug, error, info};
use tokio::{net::TcpStream, time};
use tokio_kcp::{KcpListener, KcpServerStream};

use crate::config::{Config, ServerAddr};

/// Local mode
///
/// ```plain
///        KCP (UDP)                 TCP Loopback
/// CLIENT ---------> [SSKCP-Server] <----------> [SS-Server]
/// ```
pub async fn start_proxy(config: Config) -> io::Result<()> {
    debug!("start server proxy with {:?}", config);

    let config = Arc::new(config);

    let mut listener = match config.remote_addr {
        ServerAddr::SocketAddr(sa) => KcpListener::bind(config.kcp_config.unwrap_or_default(), sa).await?,
        ServerAddr::DomainName(ref dname, port) => {
            KcpListener::bind(config.kcp_config.unwrap_or_default(), (dname.as_str(), port)).await?
        }
    };

    info!("KCP server listening on {}", listener.local_addr().unwrap());

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

async fn handle_client(config: &Config, mut stream: KcpServerStream, _peer_addr: SocketAddr) -> io::Result<()> {
    let mut local_stream = match config.local_addr {
        ServerAddr::SocketAddr(ref a) => TcpStream::connect(a).await?,
        ServerAddr::DomainName(ref dname, port) => TcpStream::connect((dname.as_str(), port)).await?,
    };

    tokio::io::copy_bidirectional(&mut stream, &mut local_stream)
        .await
        .map(|_| ())
}
