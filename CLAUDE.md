# CLAUDE.md - lazy-aws

## Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo clippy -- -D warnings  # Lint (must pass with zero warnings)
cargo fmt                # Format code
cargo fmt -- --check     # Check formatting without modifying
cargo run                # Run (no profile, shows selector)
cargo run -- --profile X --region Y  # Run with specific profile
cargo run -- --light     # Force light theme
```

Always run `cargo fmt && cargo clippy -- -D warnings` before committing.

## Architecture

```
src/
  aws/          # Domain layer - NO UI dependency
    exec.rs     # Executor trait, RealExecutor, StreamHandle, PTY streaming
    runner.rs   # Typed wrapper: list_clusters(), list_services(), list_db_instances(), etc.
    types.rs    # Cluster, Service, Task, Container, Instance, LogGroup, DbInstance...
  config/       # App config: resolve aws binary, profile, region
  credentials.rs # Saved RDS credentials (~/.config/lazy-aws/credentials.json)
  logger/       # File logger -> ~/.local/state/lazy-aws/debug.log
  ui/
    app.rs      # Main event loop - THE central file
    panels/     # 14 panel widgets (clusters, services, tasks, containers,
                #   instances, log_groups, log_streams, log_viewer,
                #   detail, output, terminal, rds_instances, rds_tables,
                #   query_results)
    components/ # 7 reusable widgets (tabbar, statusbar, spinner,
                #   confirm, choice, input, help)
    style/      # Theme system (dark/light) with dynamic color functions
    keys.rs     # KeyMap with all keybindings
    layout.rs   # Terminal layout computation
    messages.rs # Action enum
```

## Key Patterns

### Async loading (background threads + mpsc)
Every AWS call runs in its own `thread::spawn`. Results come back via `BgMsg` enum through a single `mpsc` channel. The main loop drains with `try_recv()`.

```
spawn_load_clusters() -> thread -> runner.list_clusters() -> tx.send(BgMsg::ClustersLoaded)
                                                                      |
app.run() loop -> process_bg_messages() -> bg_rx.try_recv() ----------+
```

Key spawn methods: `spawn_load_clusters`, `spawn_load_services`, `spawn_load_tasks`, `spawn_load_instances`, `spawn_load_log_groups`, `spawn_load_log_streams`, `spawn_load_caller_identity`, `spawn_load_profiles`, `spawn_check_credentials_then_load`.

### Priority-based key routing in handle_key()
Order matters - first match wins:
1. Embedded terminal (if active)
2. Confirm dialog
3. Choice dialog
4. Input box (with Ctrl+T/E/H interceptors for Insights)
5. Help popup
6. Streaming output (Ctrl+C kills)
7. Log viewer navigation (panel 2 in Logs tab)
8. Panel resize
9. Global keys (quit, help, refresh, tabs, navigation)
10. Tab-specific actions

### Theme system
Colors are functions, not constants: `theme::color_text()`, `theme::color_primary()`, etc. They return different values based on `ThemeMode::Light` vs `ThemeMode::Dark`. Toggle with `Ctrl+L` at runtime.

### Panel filter pattern
Every list panel has `filter: String`, `filtered: Vec<usize>`, `rebuild_filter()`, `visible()`, `set_filter()`. Filter is case-insensitive on the item's name/id.

### Executor trait
`exec.rs` defines `Executor` trait with `run()` (sync JSON) and `stream()` (PTY via `script -qefc`). `run()` auto-appends `--output json --profile X --region Y --no-paginate`.

## Gotchas

- **session-manager-plugin crashes in embedded PTY**: `aws ecs execute-command` and `aws ssm start-session` use suspend/resume TUI (not embedded terminal). The Go-based plugin doesn't work inside `portable-pty`.
- **OSC 11 terminal detection blocks stdin**: Removed the OSC 11 background color query because `stdin.read()` blocks indefinitely on some terminals. Theme detection uses `COLORFGBG` env var only.
- **`--no-paginate` on all run() calls**: Added to `build_args()` in exec.rs to avoid truncated results.
- **SSO token is shared across profiles**: All profiles with the same `sso_start_url` share one token. `switch_profile` tests credentials first (`spawn_check_credentials_then_load`) before triggering SSO login.
- **`aws configure list-profiles` ignores `--output json`**: Returns plain text. Parsed directly in `spawn_load_profiles` using `std::process::Command` (not the Runner).
- **Region auto-resolved from `~/.aws/config`**: `resolve_profile_region()` and `is_sso_profile()` parse the config file manually (INI format).
- **SSM tunnel process must be detached**: `std::mem::forget(child)` is used in `open_ssm_tunnel()` to prevent the `Child` from being dropped (which closes its pipes and kills the process). The tunnel is tracked by PID and killed explicitly via `kill_process()`.
- **Ctrl+Backspace sends Ctrl+H**: Most terminals send `Char('h')` + CONTROL for Ctrl+Backspace. The InputBox handles this explicitly to avoid inserting 'h'.
- **RDS API uses PascalCase with DB prefix**: Fields like `DBInstanceIdentifier` (not `DbInstanceIdentifier`) need explicit `#[serde(rename)]` — `rename_all = "PascalCase"` alone doesn't match.

## Files

- **Debug log**: `~/.local/state/lazy-aws/debug.log` (truncated on startup)
- **Saved credentials**: `~/.config/lazy-aws/credentials.json` (permissions 0600, passwords base64-encoded, keyed by profile/instance)

## app.rs Structure

The App struct has ~30 fields. Key state groups:
- **runner**: `Option<Arc<Runner>>` - None until profile selected
- **panels**: 11 panel instances
- **components**: confirm, choice, help, input, spinner
- **loading flags**: `loading_clusters`, `loading_services`, etc.
- **mode tracking**: `input_mode: InputMode`, `choice_mode: ChoiceMode`
- **pending actions**: `pending_exec`, `pending_sso_login`, `pending_install_plugin`, `pending_action: Option<PendingAction>`, `pending_shell`

## Tabs

| Tab | Index | Panels (left top/bottom) | Right panel |
|-----|-------|--------------------------|-------------|
| ECS | 0 | Clusters / Services | Detail |
| Tasks | 1 | Tasks / Containers | Detail or Terminal |
| SSM | 2 | Instances / Sessions | Detail |
| Logs | 3 | Log Groups / Log Streams | Log Viewer + Log Detail (split) |
| RDS | 4 | RDS Instances / Tables | Detail (disconnected) or Query Results (connected) |

Logs and RDS tabs use 25/75 horizontal split (vs 50/50 for other tabs).

## Testing

Tests are unit tests in each module (`#[test]`). No integration tests hitting real AWS. Mock executor not yet implemented for Runner tests.

Key test locations:
- `src/aws/exec.rs` - split_command, strip_cr
- `src/config/mod.rs` - resolve with various env var combos
- `src/ui/components/*.rs` - each component has show/hide/handle_key tests
- `src/ui/keys.rs` - all bindings populated
- `src/ui/layout.rs` - layout computation
- `src/ui/style/theme.rs` - status colors
- `src/ui/text.rs` - wrap_field

## Debug

Logs go to `~/.local/state/lazy-aws/debug.log` (truncated on each startup). All AWS CLI commands are logged at DEBUG level with full args.
