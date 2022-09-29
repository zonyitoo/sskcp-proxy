use std::{io, marker::Unpin, net::SocketAddr, sync::Arc, time::Duration};

use futures::StreamExt;
use log::{debug, error, info};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    time,
};
use tokio_kcp::KcpListener;
use tokio_yamux::{Config as YamuxConfig, Session as YamuxSession};

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

    let yamux_config = YamuxConfig::default();

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
        let mut yamux_stream = YamuxSession::new_server(stream, yamux_config.clone());
        tokio::spawn(async move {
            loop {
                let stream = match yamux_stream.next().await {
                    Some(Ok(stream)) => stream,
                    Some(Err(err)) => {
                        error!("yamux channel {} error: {}", peer_addr, err);
                        break;
                    }
                    None => {
                        debug!("yamux channel {} closed", peer_addr);
                        break;
                    }
                };

                debug!("yamux accepted stream from {}", peer_addr);

                let config = config.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_client(&config, stream, peer_addr).await {
                        error!("failed to handle client {}, error: {}", peer_addr, err);
                    }
                });
            }
        });
    }
}

async fn handle_client<S>(config: &Config, mut stream: S, _peer_addr: SocketAddr) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut local_stream = match config.local_addr {
        ServerAddr::SocketAddr(ref a) => TcpStream::connect(a).await?,
        ServerAddr::DomainName(ref dname, port) => TcpStream::connect((dname.as_str(), port)).await?,
    };

    tokio::io::copy_bidirectional(&mut stream, &mut local_stream)
        .await
        .map(|_| ())
}
