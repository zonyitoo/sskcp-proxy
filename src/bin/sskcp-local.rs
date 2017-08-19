extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_kcp;
extern crate sskcp;
extern crate env_logger;
extern crate log;
extern crate time;

use std::env;

use sskcp::config::{Config, ServerAddr};
use sskcp::local::start_proxy;
use sskcp::opt::PluginOpts;
use sskcp::plugin::{PluginConfig, PluginMode, launch_plugin};

use env_logger::LogBuilder;
use log::{LogLevelFilter, LogRecord};
use tokio_kcp::{KcpConfig, KcpNoDelayConfig};

fn log_time(record: &LogRecord) -> String {
    format!("[{}][{}] {}", time::now().strftime("%Y-%m-%d][%H:%M:%S.%f").unwrap(), record.level(), record.args())
}

fn main() {
    let mut log_builder = LogBuilder::new();
    log_builder.filter(None, LogLevelFilter::Info);
    // Default filter
    log_builder.format(log_time);
    if let Ok(env_conf) = env::var("RUST_LOG") {
        log_builder.parse(&env_conf);
    }
    log_builder.init().unwrap();

    let remote_host = env::var("SS_REMOTE_HOST").expect("Require SS_REMOTE_HOST");
    let remote_port = env::var("SS_REMOTE_PORT").expect("Require SS_REMOTE_PORT");
    let local_host = env::var("SS_LOCAL_HOST").expect("Require SS_LOCAL_HOST");
    let local_port = env::var("SS_LOCAL_PORT").expect("Require SS_LOCAL_PORT");

    let remote_port = remote_port.parse::<u16>()
                                 .expect("SS_REMOTE_PORT must be a valid port");
    let local_port = local_port.parse::<u16>()
                               .expect("SS_LOCAL_PORT must be a valid port");

    let (plugin, kcp_config) = match env::var("SS_PLUGIN_OPTIONS") {
        Err(..) => (None, None),
        Ok(opt) => {
            let opt = PluginOpts::from_str(&opt).expect("Unrecognized SS_PLUGIN_OPTIONS");

            let mut plugin = None;
            if let Some(ref o) = opt.plugin {
                plugin = Some(PluginConfig {
                                  plugin: o.clone(),
                                  plugin_opt: opt.plugin_opts.clone(),
                              })
            }

            let mut kcp_config = None;
            if opt.has_kcp_config() {
                let mut cfg = KcpConfig::default();
                cfg.mtu = opt.mtu;
                cfg.rx_minrto = opt.rx_minrto;
                cfg.fast_resend = opt.fast_resend;
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
                kcp_config = Some(cfg)
            }

            (plugin, kcp_config)
        }
    };

    let mut config = Config {
        local_addr: ServerAddr::from_str(local_host, local_port),
        remote_addr: ServerAddr::from_str(remote_host, remote_port),
        plugin: plugin,
        kcp_config: kcp_config,
    };

    let _plugin = launch_plugin(&mut config, PluginMode::Client);
    start_proxy(&config).unwrap();
}
