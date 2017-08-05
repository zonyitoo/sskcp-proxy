//! Plugin (SIP003)
//!
//! ```plain
//! +------------+                    +---------------------------+
//! |  SS Client +-- Local Loopback --+  Plugin Client (Tunnel)   +--+
//! +------------+                    +---------------------------+  |
//!                                                                  |
//!             Public Internet (Obfuscated/Transformed traffic) ==> |
//!                                                                  |
//! +------------+                    +---------------------------+  |
//! |  SS Server +-- Local Loopback --+  Plugin Server (Tunnel)   +--+
//! +------------+                    +---------------------------+
//! ```

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

use subprocess::Popen;
use subprocess::Result as PopenResult;

use config::{Config, ServerAddr};

mod ss_plugin;
mod obfs_proxy;

/// Config for plugin
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub plugin: String,
    pub plugin_opt: Option<String>,
}

/// Mode of Plugin
#[derive(Debug, Clone, Copy)]
pub enum PluginMode {
    Server,
    Client,
}

/// Plugin holder
#[derive(Debug)]
pub struct Plugin {
    addr: ServerAddr,
    process: Popen,
}

impl Plugin {
    /// Get address of the plugin
    pub fn addr(&self) -> &ServerAddr {
        &self.addr
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        debug!("Killing Plugin {:?}", self.process);
        let _ = self.process.terminate();
    }
}

/// Launch plugins in config
pub fn launch_plugin(config: &mut Config, mode: PluginMode) -> io::Result<Option<Plugin>> {
    let mut svr_addr_opt = None;
    let mut plugin = None;

    if let Some(ref c) = config.plugin {
        let loop_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let local_addr = SocketAddr::new(loop_ip, get_local_port()?);

        let svr_addr = match mode {
            PluginMode::Client => {
                // Client plugin will listen on local and relay to remote
                // So we allocated a new address for us to listen, and let the plugin listen on the original local
                // And then, set the plugin's remote to us
                let svr_addr = ServerAddr::SocketAddr(local_addr);
                let orig_local_addr = config.local_addr.listen_addr();
                info!("Started plugin \"{}\" on {} <-> {}", c.plugin, orig_local_addr, local_addr);
                match start_plugin(c, &svr_addr, orig_local_addr, mode) {
                    Err(err) => {
                        panic!("Failed to start plugin \"{}\", err: {}", c.plugin, err);
                    }
                    Ok(p) => {
                        let svr_addr = ServerAddr::SocketAddr(local_addr);
                        plugin = Some(Plugin {
                                          addr: svr_addr.clone(),
                                          process: p,
                                      });

                        // Replace addr with plugin
                        svr_addr
                    }
                }
            }
            PluginMode::Server => {
                // Server plugin will listen on remote and relay to local
                // So we allocate a loopback address for plugin's remote to listen, and then set our local address
                // to the newly allocated address
                let svr_addr = ServerAddr::SocketAddr(local_addr);
                let local_addr = config.local_addr.listen_addr();
                info!("Started plugin \"{}\" on {} <-> {}", c.plugin, svr_addr, local_addr);
                match start_plugin(c, &svr_addr, local_addr, mode) {
                    Err(err) => {
                        panic!("Failed to start plugin \"{}\", err: {}", c.plugin, err);
                    }
                    Ok(p) => {
                        plugin = Some(Plugin {
                                          addr: svr_addr.clone(),
                                          process: p,
                                      });

                        // Replace addr with plugin
                        svr_addr
                    }
                }
            }
        };

        svr_addr_opt = Some(svr_addr);
    }

    if let Some(svr_addr) = svr_addr_opt {
        config.local_addr = svr_addr;
    }

    Ok(plugin)
}

fn start_plugin(plugin: &PluginConfig,
                remote: &ServerAddr,
                local: &SocketAddr,
                mode: PluginMode)
                -> PopenResult<Popen> {
    if plugin.plugin == "obfsproxy" {
        obfs_proxy::start_plugin(plugin, remote, local, mode)
    } else {
        ss_plugin::start_plugin(plugin, remote, local, mode)
    }
}

fn get_local_port() -> io::Result<u16> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0))?;
    let addr = listener.local_addr()?;
    Ok(addr.port())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn generate_random_port() {
        let port = get_local_port().unwrap();
        println!("{:?}", port);
    }
}
