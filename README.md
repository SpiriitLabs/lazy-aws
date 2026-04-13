<p align="center">
  <img src="https://upload.wikimedia.org/wikipedia/commons/9/93/Amazon_Web_Services_Logo.svg" alt="AWS" width="150">
</p>

<h1 align="center">lazy-aws</h1>

<p align="center">
  A terminal user interface (TUI) for interacting with AWS services, inspired by <a href="https://github.com/jesseduffield/lazygit">lazygit</a>.
</p>

<p align="center">
  <strong>Beta</strong> — This project is under active development. Expect rough edges and breaking changes.
</p>

Built with [ratatui](https://ratatui.rs) + [crossterm](https://github.com/crossterm-rs/crossterm).

![Rust](https://img.shields.io/badge/rust-stable-orange)

## Features

- **ECS** — Browse clusters, services, tasks, and containers. Force new deployments, stop tasks, exec into containers
- **SSM** — List EC2 instances with SSM agent, start interactive sessions
- **CloudWatch Logs** — Browse log groups and streams, live tail in real-time, run Logs Insights queries with templates
- **Profile management** — Switch AWS profiles on the fly, automatic SSO login for SSO profiles
- **Search** — Filter any list with `/` (clusters, services, tasks, log groups...)
- **Copy** — Press `y` to copy ARNs, IDs, or full log lines to clipboard
- **Light/Dark theme** — Auto-detects terminal background, toggle with `Ctrl+L`

## Prerequisites

- Rust (stable)
- [AWS CLI v2](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html) installed and configured
- [session-manager-plugin](https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html) (optional, required for ECS exec and SSM sessions — lazy-aws can install it for you)

## Installation

### From GitHub releases (recommended)

Pre-built binaries are available on the [releases page](https://github.com/your-user/lazy-aws/releases).

Download the archive matching your platform, extract it, and move the binary to a directory in your `PATH`:

```bash
# Example for Linux x86_64 — adjust the version and asset name as needed
curl -L https://github.com/your-user/lazy-aws/releases/latest/download/lazy-aws-linux-x86_64.tar.gz \
  | tar -xz
mv lazy-aws ~/.local/bin/
```

Available assets: `lazy-aws-linux-x86_64`, `lazy-aws-linux-aarch64`, `lazy-aws-macos-x86_64`, `lazy-aws-macos-aarch64`.

### From source

```bash
git clone https://github.com/your-user/lazy-aws.git
cd lazy-aws
cargo build --release
```

The binary will be at `./target/release/lazy-aws`.

```bash
cp ./target/release/lazy-aws ~/.local/bin/
```

> Make sure `~/.local/bin` is in your `PATH`. If not, add this to your shell config:
>
> ```bash
> export PATH="$HOME/.local/bin:$PATH"
> ```

## Usage

```bash
# Launch and select a profile interactively
lazy-aws

# Launch with a specific profile and region
lazy-aws --profile my-profile --region eu-west-1

# Force light theme
lazy-aws --light
```

On first launch without arguments, a profile selector appears. If the selected profile uses SSO, the SSO login is triggered automatically.

## Configuration

### Custom AWS CLI binary

By default, lazy-aws looks for `aws` in your `PATH`. To use a different binary:

```bash
AWS_CLI_BIN=/usr/local/bin/aws lazy-aws
```

### AWS Profile & Region

lazy-aws respects the standard AWS environment variables:

- `AWS_PROFILE` — default profile
- `AWS_REGION` / `AWS_DEFAULT_REGION` — default region

When switching profiles, the region is automatically resolved from `~/.aws/config` (`region` or `sso_region` field).

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `1` `2` `3` `4` | Switch tab (ECS / Tasks / SSM / Logs) |
| `Tab` / `Shift+Tab` | Next / previous panel |
| `j` / `k` or `Up` / `Down` | Navigate up / down |
| `Enter` | Select / drill-down |
| `Backspace` | Go back |
| `/` | Filter current list |
| `Esc` | Clear filter / cancel |
| `y` | Copy selected item to clipboard |
| `p` | Switch AWS profile |
| `L` | SSO login |
| `R` | Refresh all data |
| `Ctrl+L` | Toggle light/dark theme |
| `?` | Show help |
| `q` / `Ctrl+C` | Quit |

### ECS tab

| Key | Action |
|-----|--------|
| `f` | Force new deployment (on a service) |

### Tasks tab

| Key | Action |
|-----|--------|
| `e` | Exec shell into container |
| `l` | View logs for service |
| `x` | Stop task |

### SSM tab

| Key | Action |
|-----|--------|
| `s` | Start SSM session |

### Logs tab

| Key | Action |
|-----|--------|
| `f` | Live tail (follow) |
| `i` | Logs Insights query |
| `/` | Filter logs in viewer |
| `g` / `G` | Go to top / bottom |
| `PgUp` / `PgDn` | Page up / down |

### Insights query editor

| Key | Action |
|-----|--------|
| `Ctrl+E` | Pick from query templates |
| `Ctrl+T` | Change time range |
| `Enter` | Execute query |
| `Esc` | Cancel |

### Panel resize

| Key | Action |
|-----|--------|
| `<` / `>` | Resize left/right split |
| `-` / `+` | Resize top/bottom split |

## Insights Query Templates

Press `Ctrl+E` in the query editor to pick from common templates:

| # | Template |
|---|----------|
| 1 | All logs (default) |
| 2 | Filter ERROR |
| 3 | Filter WARN |
| 4 | Filter Exception/Stacktrace |
| 5 | Count by log level |
| 6 | Top 20 error messages |
| 7 | Search keyword (interactive) |
| 8 | Latency / duration stats |
| 9 | Last 200 logs |

## Development

```bash
cargo build             # Debug build
cargo build --release   # Release build
cargo test              # Run all tests
cargo clippy            # Lint
```

Debug logs are written to `~/.local/state/lazy-aws/debug.log`.

## Architecture

```
src/
├── aws/            # Domain layer (no UI dependency)
│   ├── types.rs    # Cluster, Service, Task, Container, Instance, LogGroup...
│   ├── exec.rs     # Executor trait, RealExecutor, streaming
│   └── runner.rs   # Typed wrapper over AWS CLI
├── config/         # Config resolution, binary validation
├── logger/         # File logger (~/.local/state/lazy-aws/debug.log)
└── ui/
    ├── app.rs      # Event loop, async loading, key routing
    ├── style/      # Color theme (dark/light) + style functions
    ├── components/ # TabBar, StatusBar, ConfirmDialog, InputBox, Spinner, Help
    └── panels/     # Clusters, Services, Tasks, Containers, Instances,
                    # LogGroups, LogStreams, LogViewer, Detail, Output, Terminal
```

All data loading is asynchronous (background threads + `mpsc` channels) so the UI stays responsive. Each AWS call runs in its own thread — if one fails, the others continue.

## License

MIT
