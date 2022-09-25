use serde::{Deserialize, Serialize};
use serde_urlencoded::{self, de::Error as DeError, ser::Error as SerError};
use tokio_kcp::{KcpConfig, KcpNoDelayConfig};

#[derive(Default, Serialize, Deserialize)]
pub struct PluginOpts {
    pub mtu: Option<usize>,
    pub nodelay: Option<bool>,
    pub interval: Option<i32>,
    pub resend: Option<i32>,
    pub nc: Option<bool>,
    pub sndwnd: Option<u16>,
    pub rcvwnd: Option<u16>,
    pub stream: Option<bool>,
}

impl PluginOpts {
    pub fn from_str(opt: &str) -> Result<PluginOpts, DeError> {
        serde_urlencoded::from_str(opt)
    }

    pub fn to_string(&self) -> Result<String, SerError> {
        serde_urlencoded::to_string(self)
    }

    pub fn build_kcp_config(&self) -> KcpConfig {
        let mut kcp_config = KcpConfig::default();
        kcp_config.stream = self.stream.unwrap_or(true);
        if let Some(mtu) = self.mtu {
            kcp_config.mtu = mtu;
        }

        let mut nodelay = KcpNoDelayConfig::normal();
        if let Some(nd) = self.nodelay {
            nodelay.nodelay = nd;
        }
        if let Some(itv) = self.interval {
            nodelay.interval = itv;
        }
        if let Some(resend) = self.interval {
            nodelay.resend = resend;
        }
        if let Some(nc) = self.nc {
            nodelay.nc = nc;
        }
        kcp_config.nodelay = nodelay;

        if self.sndwnd.is_some() || self.rcvwnd.is_some() {
            kcp_config.wnd_size = (self.sndwnd.unwrap_or(4096), self.rcvwnd.unwrap_or(4096));
        } else {
            kcp_config.wnd_size = (4096, 4096);
        }

        kcp_config
    }
}
