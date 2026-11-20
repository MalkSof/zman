# zman

**English** · [Español](README.es.md)

Automatic, local-first time tracker written in Rust. An open source
alternative to RescueTime: it records which window you are using and for how
long, and keeps the data on your machine.

> Status: early development. Windows only for now.

## Privacy

- zman never records keystrokes or screen content. It only reads the active
  window's app name and title, and *how long ago* the last input happened.
- Apps and window titles you exclude are never written to disk.
- All data stays in a local SQLite file.

## Build and run

```sh
cargo build --release
```

Start the tracker and leave it running:

```sh
./target/release/zman-daemon
```

In another terminal:

```sh
./target/release/zman watch    # follow what is being recorded, live
./target/release/zman status   # file locations, device id, event count
```

Messages are shown in Spanish when the system language is Spanish, and in
English otherwise. Set the `ZMAN_LANG` environment variable (`es` or `en`) to
force one.

## Configuration

Optional. `zman status` prints where the file goes
(`%APPDATA%\zman\config\config.toml` on Windows). Restart the daemon after
editing it.

```toml
# How often the active window is sampled.
poll_interval_secs = 1
# Seconds without keyboard/mouse input before you count as away.
# Time away is not recorded.
afk_threshold_secs = 180

[exclude]
# Case-insensitive, `*` is a wildcard.
apps = ["keepassxc.exe"]
titles = ["*InPrivate*", "*Private Browsing*", "*Navegación privada*", "*banco*"]
```

If `titles` is omitted, private browsing windows of Edge and Firefox are
excluded by default.

## Layout

- `crates/core` — event model, SQLite storage, config, tracking logic.
- `crates/daemon` — background process; OS specific code sits behind the
  `Platform` trait in `src/platform`.
- `crates/cli` — the `zman` command.

## License

MIT
