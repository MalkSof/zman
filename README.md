# zman

**English** · [Español](README.es.md)

Automatic, local-first time tracker written in Rust. An open source
alternative to RescueTime: it records which app and site you are using and for
how long, shows you where your time goes, and lets your AI assistant answer
questions about it. Your data stays on your machine unless you choose to sync
it to your own server.

Website and live demo: https://malksof.github.io/zman/

> Status: early. Developed and tested on Windows. See
> [what has been verified](#what-has-been-verified).

## What is in the box

| Piece | What it does |
|---|---|
| **Desktop app** (`zman-app`) | Tracks in the background, lives in the system tray, starts with the system, shows the dashboard, blocks distracting sites and connects AI assistants |
| **CLI** (`zman`) | `today`, `week`, `apps`, `export`, `watch`, `status`, `sync` |
| **Daemon** (`zman-daemon`) | The tracker alone, for those who do not want the app |
| **Local MCP server** (`zman-mcp`, or `zman-app --mcp`) | Tools for Claude and other assistants on this computer: read-only reports, plus focus sessions if you allow it |
| **Browser extension** (`extension/`) | Adds the site of the active tab to your tracking, and blocks the sites zman tells it to |
| **Sync server** (`zman-server`) | Collects all your devices; remote MCP with sign-in for ChatGPT and Claude on the web |
| **Android app** (`android/`) | Adds your phone's app usage |

## Privacy

- zman never records keystrokes or screen content. It reads the active
  window's app name and title, and *how long ago* the last input happened.
- Apps, window titles and sites you exclude are never written to disk.
- Private browser windows are never read or reported. Query strings of URLs
  are never stored.
- AI assistants only ever receive aggregates (time per category, app and
  site). Window titles are never exposed through MCP. The only thing an
  assistant can change is starting or ending a focus session, and only after
  you turn that on.
- Nothing leaves your computer unless you sign in to a sync server, and you
  can keep window titles local even then.

## Quick start

**Windows installer**: download `zman_x.y.z_x64-setup.exe` from the
[latest release](https://github.com/MalkSof/zman/releases/latest) and run it.
The installer is not code-signed yet, so Windows SmartScreen will ask you to
confirm ("More info" → "Run anyway").

**From source**: you need [Rust](https://rustup.rs). On Windows the app uses
WebView2, which Windows 10 and 11 already include.

```sh
cargo build --release
./target/release/zman-app
```

The app opens the dashboard and keeps tracking from the tray when you close
the window. Turn on **Start with the system** in its settings menu.

Prefer the terminal? Run `./target/release/zman-daemon` instead and use the
CLI:

```sh
zman today            # hours, categories, top apps and sites of today
zman week             # the week, day by day
zman apps --week      # every app
zman watch            # what is being recorded, live
zman today --date 2026-11-19
zman export --from 2026-11-01 --to 2026-11-30 -o november.csv
```

Messages are shown in Spanish when the system language is Spanish, and in
English otherwise. Set `ZMAN_LANG` (`es` or `en`) to force one.

## Connect your AI assistant

Open the app and press **Connect**.

- **Claude Desktop**: one click, then restart Claude Desktop.
- **Claude Code**: run the command the dialog shows
  (`claude mcp add zman -- "<path>/zman-app" --mcp`).
- **ChatGPT, Claude on the web, or several devices**: sign in to a
  [sync server](#sync-server) from the same dialog. It gives you an address
  like `https://your-server/mcp`; add it as a custom connector in your
  assistant and sign in with the same email and password.

Then ask things like *"how did I spend my week?"*, *"compare this week with
the last one"* or *"when were my focus blocks yesterday?"*.

Tools: `daily_summary`, `weekly_summary`, `time_by_category`, `time_by_app`,
`compare_periods`, `focus_blocks`. On your own computer there are also
`start_focus` and `stop_focus`, which only work after you turn on **Let my AI
assistant start focus sessions** in the Focus dialog. Then *"block my
distractions for an hour"* just works.

## Focus and blocking

Press **Focus** in the app. Blocking happens in the browser, so it needs the
[browser extension](#browser-extension).

- **Focus session**: 25, 50 or 90 minutes during which distracting sites
  (every site in a category with a negative score) are replaced by a
  countdown.
- **Daily limits**: minutes per day for a site (`youtube.com`) or for a whole
  category (`Entertainment`). When they run out, it is blocked until
  tomorrow.
- **Focus hours**: distracting sites are blocked every week on the days and
  hours you choose.
- **Always blocked**: sites you never want to open.

This is a tool for self-control: it can be turned off from the app, and it
does not cover private windows or browsers without the extension. It is not
a parental control.

## Goals, notifications and categories

- Set a **daily goal** of productive hours in the Focus dialog; the day view
  shows your progress.
- zman notifies you when you reach the goal, when a focus session ends, when
  most of the last half hour went to distractions, and once a week with a
  recap. One switch turns all of it off.
- **Click any app or site** in the dashboard to move it to another category.
  The change applies to your whole history, and to the CLI and AI assistants.
- **Export this view as CSV** in the settings menu writes the events of the
  day or week on screen to your Downloads folder.

## Websites

zman reads the address of the page from the browser window itself (Chrome,
Edge, Brave, Firefox, Opera, Vivaldi), the same way a screen reader does, so
time is split per site and categorized by site with nothing to install. Only
the address is read, never the page. Private windows are skipped. Turn it off
with `read_address_bar = false`.

## Browser extension

Optional for tracking, required for [blocking](#focus-and-blocking). It
reports the exact tab, and it is what covers a blocked page.

1. Open `chrome://extensions` (or `edge://extensions`).
2. Turn on **Developer mode**.
3. **Load unpacked** and pick the `extension/` folder.

The extension talks only to zman on `127.0.0.1`. Its popup shows whether zman
is running. The release page has it as `zman-extension.zip`.

## Configuration

Optional. `zman status` prints where `config.toml` goes
(`%APPDATA%\zman\config\config.toml` on Windows). Restart the tracker after
editing it; reports pick up category changes right away.

```toml
poll_interval_secs = 1     # how often the active window is sampled
afk_threshold_secs = 180   # seconds without input before you count as away

[exclude]                  # never recorded; `*` is a wildcard, case-insensitive
apps = ["keepassxc.exe"]
titles = ["*InPrivate*", "*Private Browsing*", "*banco*"]
domains = ["mybank.com"]

[browser]
read_address_bar = true    # false: only know the site when the extension reports it
port = 27184               # 0 turns extension support off
full_urls = true           # false keeps only the site, not the page path

[sync]
titles = true              # false keeps window titles on this device

# Categories, in priority order: the first rule that matches wins.
# Within a rule, every kind of pattern given must match.
# score: -2 (very distracting) to 2 (very productive).
[[categories]]
name = "Deep work"
score = 2
apps = ["code.exe", "windowsterminal.exe"]

[[categories]]
name = "Video"
score = -2
domains = ["youtube.com", "netflix.com"]

[[categories]]
name = "Video"
score = -2
apps = ["chrome.exe"]
titles = ["*youtube*"]
```

Without `[[categories]]`, zman uses built-in rules (Development, Work,
Communication, Browsing, System, Entertainment). Defining your own replaces
them. Categories are applied when you look at the data, so changing a rule
re-categorizes your whole history.

## Sync server

Only needed to combine several devices, to use ChatGPT or Claude on the web,
or to query your data from anywhere. It needs a machine with Docker and a
domain name pointing at it.

```sh
cd deploy
cp .env.example .env     # set your domain and a database password
docker compose up -d
```

That starts the server, Postgres and Caddy with automatic HTTPS. Then, on each
computer:

```sh
zman sync login https://your-domain --register   # the first time
zman sync login https://your-domain              # other devices
```

or use **Connect** in the app. Devices upload on their own every few minutes.
Once your account exists, set `ZMAN_ALLOW_REGISTRATION=false` in `.env` and
run `docker compose up -d` again so nobody else can sign up.

To try the server without Docker or Postgres (data is lost on exit):

```sh
DATABASE_URL=memory: cargo run -p zman-server
```

## Android

`android/` is an Android Studio project. The app asks for *usage access*,
shows today's time per app, and uploads sessions to your sync server every 30
minutes. Open the folder in Android Studio and run it on a phone.

## Layout

- `crates/core` — event model, SQLite storage, config, categories, reports.
- `crates/daemon` — tracking loop, browser listener, sync client; OS specific
  code sits behind the `Platform` trait in `src/platform`.
- `crates/cli` — the `zman` command.
- `crates/mcp` — MCP protocol and tools, shared by the local and remote servers.
- `crates/desktop` — Tauri app; the dashboard is plain HTML/CSS/JS in `ui/`.
- `crates/server` — sync API, OAuth and remote MCP (Axum + Postgres).
- `site/` — source of the website; `python scripts/build_site.py` writes it to `docs/`, which GitHub Pages serves.
- `extension/` — browser extension. `android/` — Android app. `deploy/` — Docker Compose.

Demo data, to try the reports and dashboard right away:

```sh
ZMAN_HOME=./demo cargo run -p zman-core --example seed_demo
ZMAN_HOME=./demo cargo run -p zman-cli -- week
```

## What has been verified

- **Tested on Windows 11**: tracker, CLI, local MCP, desktop app, the browser
  listener and the blocking decisions (with simulated requests from the
  extension), reading the page address from Chrome (including skipping an
  incognito window; other browsers and languages are untested) and the server on in-memory storage, including the whole OAuth
  flow and remote MCP. `cargo test` covers the core
  logic, the MCP tools and the server's HTTP API.
- **Not yet run**: the server against a real Postgres, the Docker image, the
  browser extension inside a browser, the Android app (written without an
  Android SDK at hand), and connecting ChatGPT or Claude on the web to a
  deployed server. macOS and Linux need a `Platform` implementation.

## License

MIT
