use std::env;

use sskcp::{
    config::{Config, ServerAddr},
    opt::PluginOpts,
    server::start_proxy,
};

use tokio_kcp::{KcpConfig, KcpNoDelayConfig};

#[tokio::main]
async fn main() {
    env_logger::init();

    let remote_host = env::var("SS_REMOTE_HOST").expect("require SS_REMOTE_HOST");
    let remote_port = env::var("SS_REMOTE_PORT").expect("require SS_REMOTE_PORT");
    let local_host = env::var("SS_LOCAL_HOST").expect("require SS_LOCAL_HOST");
    let local_port = env::var("SS_LOCAL_PORT").expect("require SS_LOCAL_PORT");

    let remote_port = remote_port.parse::<u16>().expect("SS_REMOTE_PORT must be a valid port");
    let local_port = local_port.parse::<u16>().expect("SS_LOCAL_PORT must be a valid port");

    let kcp_config = match env::var("SS_PLUGIN_OPTIONS") {
        Err(..) => None,
        Ok(opt) => {
            let opt = PluginOpts::from_str(&opt).expect("unrecognized SS_PLUGIN_OPTIONS");

            let mut cfg = KcpConfig::default();
            // Always uses stream mode
            cfg.stream = true;

            if opt.has_kcp_config() {
                cfg.mtu = opt.mtu;
                cfg.rx_minrto = opt.rx_minrto;
                cfg.wnd_size = Some((256, 256));
                if opt.has_kcp_nodelay_config() {
                    let mut c = KcpNoDelayConfig::default();
                    if let Some(nodelay) = opt.nodelay {
                        c.nodelay = nodelay;
                    }
                    if let Some(itv) = opt.interval {
                        c.interval = itv;
                    }
                    if let Some(rs) = opt.resend {
                        c.resend = rs;
                    }
                    if let Some(nc) = opt.nc {
                        c.nc = nc;
                    }
                    cfg.nodelay = Some(c);
                }
            }

            Some(cfg)
        }
    };

    let config = Config {
        local_addr: ServerAddr::from_str(local_host, local_port),
        remote_addr: ServerAddr::from_str(remote_host, remote_port),
        kcp_config: kcp_config,
    };

    start_proxy(config).await.unwrap();
}
