## KCP Plugin for ShadowSocks

A ShadowSocks' SIP003 plugin for relaying data in [KCP](https://github.com/skywind3000/kcp) protocol.

*NOTE: Does not support standalone mode*

```plain
+-----------+                +-----------------+                +-----------------+
|    SS     | -------------- | PLUGIN LOCAL    | -------------- | SSKCP LOCAL     |
|   LOCAL   |  TCP LoopBack  | TCP IN, TCP OUT |  TCP LoopBack  | TCP IN, KCP OUT |
+-----------+                +-----------------+                +-----------------+
                                                                         |
                                                                         |
                                                                         | INTERNET
                                                                         |   UDP
                                                                         |
+-----------+                +-----------------+                +-----------------+
|    SS     | -------------- | PLUGIN SERVER   | -------------- | SSKCP SERVER    |
|  SERVER   |  TCP LoopBack  | TCP IN, TCP OUT |  TCP LoopBack  | KCP IN, TCP OUT |
+-----------+                +-----------------+                +-----------------+
```

## Usage

Use it like a normal SIP003 plugin.

```bash
$ cargo build --release
```

### Options

Plugin options passed in `SS_PLUGIN_OPTIONS` are encoded in key-value pairs with URL encodes.

* `plugin` - Secondary plugin name
* `plugin_opts` - Options for secondary plugin
* `mtu` - Maximum transmission unit
* `nodelay` - Set `true` to enable nodelay mode
* `interval` - KCP internal state update interval
* `resend` - KCP resend
* `nc` - Set `true` to disable congestion control
* `outbound_fwmark`: Linux (or Android) sockopt `SO_MARK`
* `outbound_user_cookie`: FreeBSD sockopt `SO_USER_COOKIE`
* `outbound_bind_interface`: Socket binds to interface, Linux `SO_BINDTODEVICE`, macOS `IP_BOUND_IF`, Windows `IP_UNICAST_IF`
* `outbound_bind_addr`: Socket binds to IP

Example:

- Furious mode

```plain
nodelay=true&interval=10&resend=2&nc=true
```

- Start a secondary plugin

```plain
plugin=obfs-local&plugin_opts=obfs%3dhttp%3bhost%3dwww.example.com
```

## License

MIT
