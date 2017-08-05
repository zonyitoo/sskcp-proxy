use tokio_kcp::KcpConfig;

use plugin::PluginConfig;

use std::fmt::{self, Display};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug)]
pub enum ServerAddr {
    SocketAddr(SocketAddr),
    DomainName(String, u16),
}

impl ServerAddr {
    pub fn from_str(host: String, port: u16) -> ServerAddr {
        match host.parse::<IpAddr>() {
            Ok(ip) => ServerAddr::SocketAddr(SocketAddr::new(ip, port)),
            Err(..) => ServerAddr::DomainName(host, port),
        }
    }

    pub fn listen_addr(&self) -> &SocketAddr {
        match *self {
            ServerAddr::SocketAddr(ref addr) => addr,
            ServerAddr::DomainName(..) => panic!("Listen addr can only be SocketAddr"),
        }
    }

    pub fn host(&self) -> String {
        match *self {
            ServerAddr::SocketAddr(ref addr) => addr.ip().to_string(),
            ServerAddr::DomainName(ref domain, _) => domain.clone(),
        }
    }

    pub fn port(&self) -> u16 {
        match *self {
            ServerAddr::SocketAddr(ref addr) => addr.port(),
            ServerAddr::DomainName(_, port) => port,
        }
    }
}

impl Display for ServerAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ServerAddr::SocketAddr(ref addr) => addr.fmt(f),
            ServerAddr::DomainName(ref domain, ref port) => write!(f, "{}:{}", domain, port),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub local_addr: ServerAddr,
    pub remote_addr: ServerAddr,
    pub plugin: Option<PluginConfig>,
    pub kcp_config: Option<KcpConfig>,
}
