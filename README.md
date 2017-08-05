## KCP Plugin for ShadowSocks

A ShadowSocks' SIP003 plugin for relaying data in [KCP](https://github.com/skywind3000/kcp) protocol.

*NOTE: Does not support standalone mode*

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
