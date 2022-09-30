use std::env;

use env_logger::Builder;
use sskcp::{
    config::{Config, ServerAddr},
    opt::PluginOpts,
    server::start_proxy,
};

#[tokio::main]
async fn main() {
    let mut builder = Builder::from_default_env();
    builder.format_timestamp_millis().init();

    #[cfg(all(unix, not(target_os = "android")))]
    sskcp::adjust_nofile();

    let remote_host = env::var("SS_REMOTE_HOST").expect("require SS_REMOTE_HOST");
    let remote_port = env::var("SS_REMOTE_PORT").expect("require SS_REMOTE_PORT");
    let local_host = env::var("SS_LOCAL_HOST").expect("require SS_LOCAL_HOST");
    let local_port = env::var("SS_LOCAL_PORT").expect("require SS_LOCAL_PORT");

    let remote_port = remote_port.parse::<u16>().expect("SS_REMOTE_PORT must be a valid port");
    let local_port = local_port.parse::<u16>().expect("SS_LOCAL_PORT must be a valid port");

    let mut plugin_opts = PluginOpts::default();
    if let Ok(opt) = env::var("SS_PLUGIN_OPTIONS") {
        plugin_opts = PluginOpts::from_str(&opt).expect("unrecognized SS_PLUGIN_OPTIONS");
    }

    let config = Config {
        local_addr: ServerAddr::from_str(local_host, local_port),
        remote_addr: ServerAddr::from_str(remote_host, remote_port),
        kcp_config: plugin_opts.build_kcp_config(),
        plugin_opts,
    };

    start_proxy(config).await.unwrap();
}
