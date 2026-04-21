use std::io;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};
use ratatui::Terminal;

use crate::aws::exec::kill_process;
use crate::aws::{self, Runner, StreamLine};
use crate::credentials;
use crate::ui::components::*;
use crate::ui::keys::default_key_map;
use crate::ui::layout::{compute_layout, LayoutMode};
use crate::ui::messages::Action;
use crate::ui::panels;
use crate::ui::style::{styles, theme};

const TAB_ECS: usize = 0;
const TAB_TASKS: usize = 1;
const TAB_SSM: usize = 2;
const TAB_LOGS: usize = 3;
const TAB_RDS: usize = 4;
const TAB_S3: usize = 5;

/// Messages from background threads.
enum BgMsg {
    CallerIdentityLoaded(aws::CallerIdentity),
    CallerIdentityError(String),
    ClustersLoaded(Vec<aws::Cluster>),
    ClustersError(String),
    ServicesLoaded {
        services: Vec<aws::Service>,
    },
    ServicesError(String),
    TasksLoaded {
        tasks: Vec<aws::Task>,
    },
    TasksError(String),
    InstancesLoaded(Vec<aws::Instance>),
    InstancesError(String),
    LogGroupsLoaded(Vec<aws::LogGroup>),
    LogGroupsError(String),
    LogStreamsLoaded {
        streams: Vec<aws::LogStream>,
    },
    LogStreamsError(String),
    AwsInfo {
        version: String,
    },
    ProfilesLoaded(Vec<String>),
    ProfilesError(String),
    InsightsResults {
        status: String,
        results: Vec<Vec<(String, String)>>,
    },
    InsightsError(String),
    CredentialsValid,
    CredentialsExpired,
    DbInstancesLoaded(Vec<aws::DbInstance>),
    DbInstancesError(String),
    RdsConnectionOk,
    RdsConnectionError(String),
    RdsTablesLoaded(Vec<String>),
    RdsTablesError(String),
    QueryResult {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        duration_ms: u64,
        query: String,
    },
    QueryError(String),
    DmlPreview {
        sql: String,
        affected_rows: u64,
    },
    DmlPreviewError(String),
    SsmTunnelReady {
        pid: u32,
    },
    SsmTunnelError(String),
    SsmInstancesForTunnel(Vec<aws::Instance>),
    SsmInstancesForTunnelError(String),
    BucketsLoaded(Vec<aws::Bucket>),
    BucketsError(String),
    ObjectsLoaded {
        result: aws::S3ListResult,
        bucket: String,
        prefix: String,
    },
    ObjectsError(String),
    DeleteObjectDone,
    DeleteObjectError(String),
}

#[derive(PartialEq)]
enum ChoiceMode {
    ProfileSelector,
    TimeRangeSelector,
    QueryTemplate,
    QueryHistory,
    ShellSelector,
    RdsConnectMethod,
    SqlHistory,
    SsmInstanceSelector,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum ResizeMode {
    #[default]
    Inactive,
    Active,
}

#[derive(PartialEq)]
enum InputMode {
    None,
    InsightsQuery,
    LogFilter,
    CustomDateStart,
    CustomDateEnd,
    PanelFilter,
    KeywordSearch,
    ExportLogs,
    RdsUsername,
    RdsPassword,
    RdsDatabase,
    SqlQuery,
    SqlModify,
    ExportQueryResults,
    ImportSql,
    S3Upload,
}

struct RdsConnection {
    host: String,
    port: i32,
    user: String,
    password: String,
    database: Option<String>,
}

#[derive(Clone)]
enum TimeRange {
    Relative(i64),                     // seconds ago
    Absolute { start: i64, end: i64 }, // unix timestamps
}

impl TimeRange {
    fn to_timestamps(&self) -> (i64, i64) {
        match self {
            TimeRange::Relative(secs) => {
                let now = chrono::Utc::now().timestamp();
                (now - secs, now)
            }
            TimeRange::Absolute { start, end } => (*start, *end),
        }
    }

    fn label(&self) -> String {
        match self {
            TimeRange::Relative(secs) => match secs {
                900 => "15 minutes".to_string(),
                3600 => "1 hour".to_string(),
                21600 => "6 hours".to_string(),
                86400 => "24 hours".to_string(),
                172800 => "48 hours".to_string(),
                604800 => "7 days".to_string(),
                _ => format!("{} seconds", secs),
            },
            TimeRange::Absolute { start, end } => {
                let s = chrono::DateTime::from_timestamp(*start, 0)
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| start.to_string());
                let e = chrono::DateTime::from_timestamp(*end, 0)
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| end.to_string());
                format!("{s} → {e}")
            }
        }
    }
}

enum PendingAction {
    ForceDeploy {
        cluster: String,
        service: String,
    },
    StopTask {
        cluster: String,
        task: String,
    },
    InstallSessionPlugin,
    SaveRdsCredentials {
        profile: String,
        identifier: String,
        username: String,
        password: String,
        database: Option<String>,
    },
    DeleteS3Object {
        bucket: String,
        key: String,
    },
    ExecuteDmlQuery {
        sql: String,
    },
}

pub struct App {
    runner: Option<Arc<Runner>>,
    aws_bin: String,
    layout: crate::ui::layout::Layout,

    // Panels
    clusters: panels::ClustersPanel,
    services: panels::ServicesPanel,
    tasks: panels::TasksPanel,
    containers: panels::ContainersPanel,
    instances: panels::InstancesPanel,
    log_groups: panels::LogGroupsPanel,
    log_streams: panels::LogStreamsPanel,
    log_viewer: panels::LogViewerPanel,
    output: panels::OutputPanel,
    detail: panels::DetailPanel,
    terminal: panels::TerminalPanel,
    rds_instances: panels::RdsInstancesPanel,
    rds_tables: panels::RdsTablesPanel,
    query_results: panels::QueryResultsPanel,
    buckets: panels::BucketsPanel,
    objects: panels::ObjectsPanel,

    // Components
    _status_bar: StatusBar,
    confirm: ConfirmDialog,
    choice: ChoiceDialog,
    help: HelpPopup,
    input: InputBox,
    spinner: LoadingSpinner,

    // State
    active_tab: usize,
    active_panel: usize,   // 0 = top panel, 1 = bottom panel
    split_horizontal: u16, // left panel percentage (20..80)
    split_vertical: u16,   // top panel percentage (20..80)
    bg_rx: mpsc::Receiver<BgMsg>,
    bg_tx: mpsc::Sender<BgMsg>,
    stream_rx: Option<mpsc::Receiver<StreamLine>>,
    child_pid: Option<u32>,
    loading: bool,
    err: Option<String>,
    info: Option<String>,
    msg_time: Option<std::time::Instant>, // when err/info was set

    // Auto-refresh
    last_refresh: std::time::Instant,

    // AWS state
    aws_info: String,
    active_profile: Option<String>,
    active_region: String,
    caller_identity: Option<aws::CallerIdentity>,
    selected_cluster: Option<String>,
    selected_service: Option<String>,
    selected_bucket: Option<String>,

    // Loading flags
    loading_clusters: bool,
    loading_services: bool,
    loading_tasks: bool,
    loading_instances: bool,
    loading_log_groups: bool,
    loading_log_streams: bool,
    loading_rds_instances: bool,
    loading_query: bool,
    loading_rds_tables: bool,
    loading_buckets: bool,
    loading_objects: bool,

    // Tab visit flags (for lazy loading)
    ssm_visited: bool,
    logs_visited: bool,
    rds_visited: bool,
    s3_visited: bool,

    // RDS connection
    rds_connection: Option<RdsConnection>,
    sql_history: Vec<String>,
    last_sql_query: String,
    pending_rds_user: String,
    pending_rds_password: String,
    saved_credentials: credentials::SavedCredentials,
    credentials_just_saved: bool, // avoid re-prompting after save
    ssm_tunnel_pid: Option<u32>,
    ssm_tunnel_local_port: Option<i32>,
    pending_ssm_tunnel: bool,
    tunnel_ssm_instances: Vec<aws::Instance>, // SSM instances available for tunneling

    // Session manager plugin
    session_plugin_installed: bool,
    pending_install_plugin: bool,

    // Live tail state
    live_tail_active: bool,

    // Pending actions (waiting for confirm dialog)
    pending_action: Option<PendingAction>,

    // Mode tracking
    choice_mode: ChoiceMode,
    input_mode: InputMode,
    last_insights_query: String,
    query_history: Vec<String>,
    pending_shell: Option<String>,
    insights_time_range: TimeRange,
    custom_date_start: String, // temp storage during custom date input

    // RDS modify (DML/DDL)
    last_sql_modify_query: String,
    pending_dml_refresh: bool,

    // RDS import
    pending_import_sql: bool,
    pending_import_path: Option<String>,

    // Profile management
    available_profiles: Vec<String>,
    pending_sso_login: bool,
    pending_exec: bool,

    // Mouse hit zones (updated each render)
    hit_tab_bar: Rect,
    hit_top_panel: Rect,
    hit_bottom_panel: Rect,
    hit_right_panel: Rect,

    // Layout adaptation
    layout_mode_override: Option<LayoutMode>,
    resize_mode: ResizeMode,
}

impl App {
    pub fn new(aws_bin: String, profile: Option<String>, region: String) -> Self {
        let (bg_tx, bg_rx) = mpsc::channel();

        let runner = profile.as_ref().map(|p| {
            let exec = crate::aws::RealExecutor::new(&aws_bin, p, &region);
            Arc::new(Runner::new(Box::new(exec)))
        });

        App {
            runner,
            aws_bin,
            layout: crate::ui::layout::Layout::default(),
            clusters: panels::ClustersPanel::new(),
            services: panels::ServicesPanel::new(),
            tasks: panels::TasksPanel::new(),
            containers: panels::ContainersPanel::new(),
            instances: panels::InstancesPanel::new(),
            log_groups: panels::LogGroupsPanel::new(),
            log_streams: panels::LogStreamsPanel::new(),
            log_viewer: panels::LogViewerPanel::new(),
            output: panels::OutputPanel::new(),
            detail: panels::DetailPanel::new(),
            terminal: panels::TerminalPanel::new(),
            rds_instances: panels::RdsInstancesPanel::new(),
            rds_tables: panels::RdsTablesPanel::new(),
            query_results: panels::QueryResultsPanel::new(),
            buckets: panels::BucketsPanel::new(),
            objects: panels::ObjectsPanel::new(),
            _status_bar: StatusBar::new(),
            confirm: ConfirmDialog::new(),
            choice: ChoiceDialog::new(),
            help: HelpPopup::new(),
            input: InputBox::new(),
            spinner: LoadingSpinner::new(),
            active_tab: TAB_ECS,
            active_panel: 0,
            split_horizontal: 50,
            split_vertical: 60,
            bg_rx,
            bg_tx,
            stream_rx: None,
            child_pid: None,
            loading: false,
            err: None,
            info: None,
            msg_time: None,
            last_refresh: std::time::Instant::now(),
            aws_info: String::new(),
            active_profile: profile,
            active_region: region,
            caller_identity: None,
            selected_cluster: None,
            selected_service: None,
            selected_bucket: None,
            loading_clusters: false,
            loading_services: false,
            loading_tasks: false,
            loading_instances: false,
            loading_log_groups: false,
            loading_log_streams: false,
            loading_rds_instances: false,
            loading_query: false,
            loading_rds_tables: false,
            loading_buckets: false,
            loading_objects: false,
            ssm_visited: false,
            logs_visited: false,
            rds_visited: false,
            s3_visited: false,
            rds_connection: None,
            sql_history: Vec::new(),
            last_sql_query: String::new(),
            pending_rds_user: String::new(),
            pending_rds_password: String::new(),
            saved_credentials: credentials::load(),
            credentials_just_saved: false,
            ssm_tunnel_pid: None,
            ssm_tunnel_local_port: None,
            pending_ssm_tunnel: false,
            tunnel_ssm_instances: vec![],
            session_plugin_installed: which::which("session-manager-plugin").is_ok(),
            pending_install_plugin: false,
            live_tail_active: false,
            pending_action: None,
            choice_mode: ChoiceMode::ProfileSelector,
            input_mode: InputMode::None,
            last_insights_query: "fields @timestamp, @message | sort @timestamp desc".to_string(),
            query_history: Vec::new(),
            pending_shell: None,
            insights_time_range: TimeRange::Relative(3600), // default 1h
            custom_date_start: String::new(),
            last_sql_modify_query: String::new(),
            pending_dml_refresh: false,
            pending_import_sql: false,
            pending_import_path: None,
            available_profiles: vec![],
            pending_sso_login: false,
            pending_exec: false,
            hit_tab_bar: Rect::default(),
            hit_top_panel: Rect::default(),
            hit_bottom_panel: Rect::default(),
            hit_right_panel: Rect::default(),
            layout_mode_override: None,
            resize_mode: ResizeMode::Inactive,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Load available profiles (always)
        self.spawn_load_profiles();

        // If a profile is already set, load AWS data
        if self.active_profile.is_some() {
            self.spawn_load_clusters();
            self.spawn_load_caller_identity();
            self.spawn_load_aws_info();
        }

        loop {
            terminal.draw(|f| self.render(f))?;

            self.process_bg_messages();

            // Drain streaming channel
            if let Some(rx) = &self.stream_rx {
                let mut got_done = false;
                while let Ok(line) = rx.try_recv() {
                    if line.done {
                        if let Some(err_msg) = line.err {
                            if self.live_tail_active {
                                self.log_viewer.append_line(&format!("Error: {err_msg}"));
                            } else {
                                self.output.append_line(&format!("Error: {err_msg}"));
                            }
                        } else if self.live_tail_active {
                            self.log_viewer.append_line("-- tail ended --");
                        } else {
                            self.output.append_line("Done");
                        }
                        got_done = true;
                        break;
                    } else if self.live_tail_active {
                        self.log_viewer.append_line(&line.text);
                    } else {
                        self.output.append_line(&line.text);
                    }
                }
                if got_done {
                    self.loading = false;
                    self.stream_rx = None;
                    self.child_pid = None;
                    self.live_tail_active = false;
                    self.spinner.stop();
                }
            }

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
                        if self.handle_key(key) {
                            break;
                        }
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse);
                    }
                    Event::Resize(w, h) => {
                        self.layout = compute_layout(w, h, self.layout_mode_override);
                    }
                    _ => {}
                }
            }

            // Handle SSO login (suspend TUI)
            if self.pending_sso_login {
                self.pending_sso_login = false;
                self.run_sso_login_interactive(&mut terminal)?;
            }

            // Handle session-manager-plugin installation (suspend TUI)
            if self.pending_install_plugin {
                self.pending_install_plugin = false;
                self.run_install_session_plugin(&mut terminal)?;
            }

            // Handle exec into container (suspend TUI -- session-manager-plugin
            // doesn't work in embedded PTY)
            if self.pending_exec {
                self.pending_exec = false;
                self.run_exec_interactive(&mut terminal)?;
            }

            // Handle SQL import (suspend TUI)
            if self.pending_import_sql {
                self.pending_import_sql = false;
                self.run_import_sql_interactive(&mut terminal)?;
            }

            // Auto-refresh every 30 seconds (only if not loading)
            if self.last_refresh.elapsed() >= Duration::from_secs(30)
                && !self.loading
                && self.stream_rx.is_none()
                && self.active_profile.is_some()
            {
                self.last_refresh = std::time::Instant::now();
                self.refresh_current_tab();
            }

            self.spinner.tick();
        }

        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn process_bg_messages(&mut self) {
        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMsg::CallerIdentityLoaded(identity) => {
                    log::info!("logged in as {}", identity.arn);
                    self.caller_identity = Some(identity);
                }
                BgMsg::CallerIdentityError(e) => {
                    log::warn!("caller identity error: {e}");
                    self.handle_aws_error(&e);
                }
                BgMsg::ClustersLoaded(clusters) => {
                    self.loading_clusters = false;
                    self.clusters.set_clusters(clusters);
                    if !self.loading_services && !self.loading_tasks {
                        self.spinner.stop();
                    }
                }
                BgMsg::ClustersError(e) => {
                    self.loading_clusters = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::ServicesLoaded { services } => {
                    self.loading_services = false;
                    self.services.set_services(services);
                    if !self.loading_clusters {
                        self.spinner.stop();
                    }
                }
                BgMsg::ServicesError(e) => {
                    self.loading_services = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::TasksLoaded { tasks } => {
                    self.loading_tasks = false;
                    self.tasks.set_tasks(tasks);
                    self.spinner.stop();
                }
                BgMsg::TasksError(e) => {
                    self.loading_tasks = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::InstancesLoaded(instances) => {
                    self.loading_instances = false;
                    self.instances.set_instances(instances);
                    self.spinner.stop();
                }
                BgMsg::InstancesError(e) => {
                    self.loading_instances = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::LogGroupsLoaded(groups) => {
                    self.loading_log_groups = false;
                    self.log_groups.set_groups(groups);
                    self.spinner.stop();
                }
                BgMsg::LogGroupsError(e) => {
                    self.loading_log_groups = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::LogStreamsLoaded { streams } => {
                    self.loading_log_streams = false;
                    self.log_streams.set_streams(streams);
                    self.spinner.stop();
                }
                BgMsg::LogStreamsError(e) => {
                    self.loading_log_streams = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::AwsInfo { version } => {
                    self.aws_info = version;
                }
                BgMsg::ProfilesLoaded(profiles) => {
                    self.available_profiles = profiles;
                    // Auto-show profile selector if no profile is set
                    if self.active_profile.is_none() && !self.available_profiles.is_empty() {
                        self.show_profile_selector();
                    }
                }
                BgMsg::ProfilesError(e) => {
                    log::warn!("failed to load profiles: {e}");
                }
                BgMsg::InsightsResults { status, results } => {
                    self.spinner.stop();
                    self.log_viewer.append_line(&format!("Status: {status}"));
                    for row in &results {
                        let line: Vec<String> =
                            row.iter().map(|(k, v)| format!("{k}={v}")).collect();
                        self.log_viewer.append_line(&line.join(" | "));
                    }
                    if status == "Complete" || status == "Failed" || status == "Cancelled" {
                        self.log_viewer
                            .append_line(&format!("\n{} results", results.len()));
                        // Focus on log viewer
                        self.active_panel = 2;
                        self.log_viewer.go_to_top();
                    }
                }
                BgMsg::InsightsError(e) => {
                    self.spinner.stop();
                    self.set_error(format!("Insights: {e}"));
                    self.log_viewer.append_line(&format!("Error: {e}"));
                }
                BgMsg::DbInstancesLoaded(instances) => {
                    self.loading_rds_instances = false;
                    self.rds_instances.set_instances(instances);
                    self.spinner.stop();
                }
                BgMsg::DbInstancesError(e) => {
                    self.loading_rds_instances = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::RdsConnectionOk => {
                    self.spinner.stop();
                    self.set_info("Connected".to_string());
                    self.spawn_load_rds_tables();

                    // Prompt to save credentials if not already saved
                    if !self.credentials_just_saved {
                        if let (Some(inst), Some(conn)) =
                            (self.rds_instances.selected(), &self.rds_connection)
                        {
                            let profile =
                                self.active_profile.clone().unwrap_or("default".to_string());
                            let identifier = inst.db_instance_identifier.clone();
                            self.pending_action = Some(PendingAction::SaveRdsCredentials {
                                profile,
                                identifier,
                                username: conn.user.clone(),
                                password: conn.password.clone(),
                                database: conn.database.clone(),
                            });
                            self.confirm.show("Save credentials for this instance?");
                        }
                    }
                }
                BgMsg::RdsConnectionError(e) => {
                    self.spinner.stop();
                    self.rds_connection = None;
                    self.set_error(format!("Connection failed: {e}"));
                }
                BgMsg::RdsTablesLoaded(tables) => {
                    self.loading_rds_tables = false;
                    self.rds_tables.set_tables(tables);
                    self.spinner.stop();
                }
                BgMsg::RdsTablesError(e) => {
                    self.loading_rds_tables = false;
                    self.set_error(format!("Failed to load tables: {e}"));
                    self.spinner.stop();
                }
                BgMsg::QueryResult {
                    columns,
                    rows,
                    duration_ms,
                    query,
                } => {
                    self.loading_query = false;
                    self.query_results
                        .set_results(columns, rows, query, duration_ms);
                    self.spinner.stop();
                    // Focus on right panel to see results
                    self.active_panel = 2;
                    if self.pending_dml_refresh {
                        self.pending_dml_refresh = false;
                        self.spawn_load_rds_tables();
                    }
                }
                BgMsg::QueryError(e) => {
                    self.loading_query = false;
                    self.query_results.set_error(e);
                    self.spinner.stop();
                    self.active_panel = 2;
                    if self.pending_dml_refresh {
                        self.pending_dml_refresh = false;
                    }
                }
                BgMsg::DmlPreview { sql, affected_rows } => {
                    self.loading_query = false;
                    self.spinner.stop();
                    let truncated = if sql.len() > 80 {
                        format!("{}...", &sql[..80])
                    } else {
                        sql.clone()
                    };
                    let rows_label = if affected_rows <= 1 {
                        "ligne"
                    } else {
                        "lignes"
                    };
                    self.confirm.show(&format!(
                        "Cette action va affecter {} {}.\nExecuter ?\n\n{}",
                        affected_rows, rows_label, truncated
                    ));
                    self.pending_action = Some(PendingAction::ExecuteDmlQuery { sql });
                }
                BgMsg::DmlPreviewError(e) => {
                    self.loading_query = false;
                    self.spinner.stop();
                    self.query_results.set_error(format!("Preview failed: {e}"));
                    self.active_panel = 2;
                }
                BgMsg::SsmTunnelReady { pid } => {
                    self.spinner.stop();
                    self.pending_ssm_tunnel = false;
                    self.ssm_tunnel_pid = Some(pid);
                    log::info!("SSM tunnel ready, pid={pid}");
                    self.set_info(format!(
                        "Tunnel open on port {}",
                        self.ssm_tunnel_local_port.unwrap_or(0)
                    ));
                    // Now proceed with the username/password flow
                    self.start_rds_connect_flow();
                }
                BgMsg::SsmTunnelError(e) => {
                    self.spinner.stop();
                    self.pending_ssm_tunnel = false;
                    self.ssm_tunnel_local_port = None;
                    self.set_error(format!("SSM tunnel: {e}"));
                }
                BgMsg::SsmInstancesForTunnel(instances) => {
                    self.spinner.stop();
                    self.tunnel_ssm_instances = instances;
                    self.show_ssm_instance_selector();
                }
                BgMsg::SsmInstancesForTunnelError(e) => {
                    self.spinner.stop();
                    self.set_error(format!("Failed to load SSM instances: {e}"));
                }
                BgMsg::CredentialsValid => {
                    self.spinner.stop();
                    log::info!("credentials valid, loading data");
                    self.set_info("Credentials OK".to_string());
                    self.spawn_load_clusters();
                }
                BgMsg::CredentialsExpired => {
                    self.spinner.stop();
                    log::info!("credentials expired, triggering SSO login");
                    self.pending_sso_login = true;
                }
                BgMsg::BucketsLoaded(buckets) => {
                    self.loading_buckets = false;
                    self.buckets.set_buckets(buckets);
                    self.spinner.stop();
                }
                BgMsg::BucketsError(e) => {
                    self.loading_buckets = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::ObjectsLoaded {
                    result,
                    bucket,
                    prefix,
                } => {
                    // Only apply if still relevant (user may have navigated away)
                    if self.selected_bucket.as_deref() == Some(&bucket)
                        && self.objects.current_prefix == prefix
                    {
                        self.loading_objects = false;
                        self.objects.set_result(result);
                        self.spinner.stop();
                    }
                }
                BgMsg::ObjectsError(e) => {
                    self.loading_objects = false;
                    self.handle_aws_error(&e);
                    self.spinner.stop();
                }
                BgMsg::DeleteObjectDone => {
                    self.spinner.stop();
                    self.set_info("Object deleted".to_string());
                    self.spawn_load_objects();
                }
                BgMsg::DeleteObjectError(e) => {
                    self.spinner.stop();
                    self.set_error(format!("Delete failed: {e}"));
                }
            }
        }
    }

    /// Returns true if the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Auto-clear messages after 3 seconds
        if let Some(t) = self.msg_time {
            if t.elapsed() >= Duration::from_secs(3) {
                self.err = None;
                self.info = None;
                self.msg_time = None;
            }
        }

        let km = default_key_map();

        // Priority-based key routing

        // 1. Confirm dialog
        if self.confirm.is_visible() {
            self.confirm.handle_key(key);
            if !self.confirm.is_visible() && self.confirm.confirmed {
                self.execute_pending_action();
            }
            return false;
        }

        // 2. Choice dialog
        if self.choice.is_visible() {
            if let Some(c) = self.choice.handle_key(key) {
                if c != '\x1b' {
                    match self.choice_mode {
                        ChoiceMode::ProfileSelector => {
                            let idx = (c as u8).wrapping_sub(b'1') as usize;
                            if idx < self.available_profiles.len() {
                                let profile = self.available_profiles[idx].clone();
                                self.switch_profile(&profile);
                            }
                        }
                        ChoiceMode::TimeRangeSelector => {
                            self.handle_time_range_choice(c);
                        }
                        ChoiceMode::QueryTemplate => {
                            self.apply_query_template(c);
                        }
                        ChoiceMode::QueryHistory => {
                            let idx = (c as u8).wrapping_sub(b'1') as usize;
                            if idx < self.query_history.len() {
                                self.last_insights_query = self.query_history[idx].clone();
                                self.show_insights_query_input();
                            }
                        }
                        ChoiceMode::ShellSelector => {
                            let shell = match c {
                                '1' => "/bin/sh",
                                '2' => "/bin/bash",
                                '3' => "/bin/zsh",
                                _ => return false,
                            };
                            self.pending_shell = Some(shell.to_string());
                            self.pending_exec = true;
                        }
                        ChoiceMode::RdsConnectMethod => match c {
                            '1' => self.start_rds_connect_flow(),
                            '2' => self.start_ssm_tunnel_flow(),
                            _ => {}
                        },
                        ChoiceMode::SsmInstanceSelector => {
                            let idx = (c as u8).wrapping_sub(b'1') as usize;
                            if idx < self.tunnel_ssm_instances.len() {
                                let target = self.tunnel_ssm_instances[idx].id.clone();
                                self.open_ssm_tunnel(&target);
                            }
                        }
                        ChoiceMode::SqlHistory => {
                            let idx = (c as u8).wrapping_sub(b'1') as usize;
                            if idx < self.sql_history.len() {
                                self.last_sql_query = self.sql_history[idx].clone();
                                self.show_sql_query_input();
                            }
                        }
                    }
                }
            }
            return false;
        }

        // 3. Input box
        if self.input.is_visible() {
            if self.input_mode == InputMode::InsightsQuery
                && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                match key.code {
                    // Ctrl+T: open time range picker
                    KeyCode::Char('t') => {
                        self.last_insights_query = self.input.value();
                        self.input.hide();
                        self.show_time_range_selector();
                        return false;
                    }
                    // Ctrl+E: open query templates
                    KeyCode::Char('e') => {
                        self.last_insights_query = self.input.value();
                        self.input.hide();
                        self.show_query_templates();
                        return false;
                    }
                    // Ctrl+H: open query history
                    KeyCode::Char('h') => {
                        if !self.query_history.is_empty() {
                            self.last_insights_query = self.input.value();
                            self.input.hide();
                            self.show_query_history();
                        }
                        return false;
                    }
                    _ => {}
                }
            }
            if let Some(action) = self.input.handle_key(key) {
                self.handle_action(action);
            }
            return false;
        }

        // 4. Help popup
        if self.help.is_visible() {
            self.help.handle_key(key);
            return false;
        }

        // 5. Streaming output — Ctrl+C kills, Esc hides
        if self.stream_rx.is_some() {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                if let Some(pid) = self.child_pid {
                    kill_process(pid);
                }
                return false;
            }
            if key.code == KeyCode::Esc {
                if let Some(pid) = self.child_pid {
                    kill_process(pid);
                }
                self.stream_rx = None;
                self.child_pid = None;
                self.loading = false;
                self.spinner.stop();
                return false;
            }
            return false;
        }

        // 6. Resize mode — exclusive: arrows adjust splits, Esc/toggle exits.
        //    Only meaningful in horizontal layout; in vertical mode the toggle is a no-op.
        if km.resize_mode.matches(&key) {
            if self.layout.mode == LayoutMode::Horizontal {
                self.resize_mode = match self.resize_mode {
                    ResizeMode::Inactive => ResizeMode::Active,
                    ResizeMode::Active => ResizeMode::Inactive,
                };
            } else {
                self.info = Some("Resize disabled in vertical layout".to_string());
                self.msg_time = Some(std::time::Instant::now());
            }
            return false;
        }
        if self.resize_mode == ResizeMode::Active {
            match key.code {
                KeyCode::Left => {
                    self.split_horizontal = self.split_horizontal.saturating_sub(2).max(20);
                }
                KeyCode::Right => {
                    self.split_horizontal = (self.split_horizontal + 2).min(80);
                }
                KeyCode::Up => {
                    self.split_vertical = self.split_vertical.saturating_sub(2).max(20);
                }
                KeyCode::Down => {
                    self.split_vertical = (self.split_vertical + 2).min(80);
                }
                KeyCode::Esc => {
                    self.resize_mode = ResizeMode::Inactive;
                }
                _ => {}
            }
            return false;
        }

        // 7. Layout toggle (auto ↔ force-horizontal ↔ force-vertical).
        if km.layout_toggle.matches(&key) {
            self.layout_mode_override = match self.layout_mode_override {
                None => Some(LayoutMode::Horizontal),
                Some(LayoutMode::Horizontal) => Some(LayoutMode::Vertical),
                Some(LayoutMode::Vertical) => None,
            };
            return false;
        }

        // 8. Global keys
        if km.quit.matches(&key) {
            return true;
        }
        if km.help.matches(&key) {
            self.help.show();
            return false;
        }
        if km.refresh.matches(&key) {
            self.refresh_current_tab();
            return false;
        }

        // Tab switching
        if km.tab_ecs.matches(&key) {
            self.switch_tab(TAB_ECS);
            return false;
        }
        if km.tab_tasks.matches(&key) {
            self.switch_tab(TAB_TASKS);
            return false;
        }
        if km.tab_ssm.matches(&key) {
            self.switch_tab(TAB_SSM);
            return false;
        }
        if km.tab_logs.matches(&key) {
            self.switch_tab(TAB_LOGS);
            return false;
        }
        if km.tab_rds.matches(&key) {
            self.switch_tab(TAB_RDS);
            return false;
        }
        if km.tab_s3.matches(&key) {
            self.switch_tab(TAB_S3);
            return false;
        }

        // Panel switching within tab
        if km.next_tab.matches(&key) {
            let max_panels = self.max_panels();
            self.active_panel = (self.active_panel + 1) % max_panels;
            return false;
        }
        if km.prev_tab.matches(&key) {
            let max_panels = self.max_panels();
            self.active_panel = if self.active_panel == 0 {
                max_panels - 1
            } else {
                self.active_panel - 1
            };
            return false;
        }

        // Log viewer navigation (when focused on log viewer, panel 2 in Logs tab)
        if self.active_tab == TAB_LOGS && self.active_panel == 2 {
            match key.code {
                KeyCode::Char('g') => {
                    self.log_viewer.go_to_top();
                    return false;
                }
                KeyCode::Char('G') => {
                    self.log_viewer.go_to_bottom();
                    return false;
                }
                KeyCode::Char('/') => {
                    self.input_mode = InputMode::LogFilter;
                    let current = &self.log_viewer.filter;
                    if current.is_empty() {
                        self.input
                            .show("Filter logs (case-insensitive)", "type to filter...");
                    } else {
                        self.input.show_with_value(
                            "Filter logs (case-insensitive)",
                            "type to filter...",
                            current,
                        );
                    }
                    return false;
                }
                KeyCode::Esc => {
                    if !self.log_viewer.filter.is_empty() {
                        // Clear filter
                        self.log_viewer.clear_filter();
                        return false;
                    }
                    // Otherwise Esc goes back
                }
                KeyCode::PageUp => {
                    self.log_viewer.page_up();
                    return false;
                }
                KeyCode::PageDown => {
                    self.log_viewer.page_down();
                    return false;
                }
                _ => {}
            }
        }

        // Query results navigation (when focused on query results, panel 2 in RDS tab)
        if self.active_tab == TAB_RDS && self.active_panel == 2 {
            match key.code {
                KeyCode::Char('h') | KeyCode::Left => {
                    self.query_results.scroll_left();
                    return false;
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.query_results.scroll_right();
                    return false;
                }
                _ => {}
            }
        }

        // Search/filter in panels (/)
        if km.search.matches(&key) && !(self.active_tab == TAB_LOGS && self.active_panel == 2) {
            self.input_mode = InputMode::PanelFilter;
            let current = self.get_active_panel_filter();
            if current.is_empty() {
                self.input.show("Filter", "type to filter...");
            } else {
                self.input
                    .show_with_value("Filter", "type to filter...", &current);
            }
            return false;
        }

        // Esc to clear panel filter
        if km.escape.matches(&key) && self.clear_active_panel_filter() {
            return false;
        }

        // Detail panel scroll (Ctrl+Up/Down)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Up => {
                    self.detail.scroll_up();
                    return false;
                }
                KeyCode::Down => {
                    let h = self.hit_right_panel.height.saturating_sub(2);
                    self.detail.scroll_down(h);
                    return false;
                }
                _ => {}
            }
        }

        // Navigation
        if km.up.matches(&key) {
            self.navigate_up();
            return false;
        }
        if km.down.matches(&key) {
            self.navigate_down();
            return false;
        }
        if km.enter.matches(&key) {
            self.handle_enter();
            return false;
        }
        if km.back.matches(&key) {
            self.handle_back();
            return false;
        }

        // Yank/copy (y)
        if km.yank.matches(&key) {
            self.handle_yank();
            return false;
        }

        // Profile switch (p)
        if km.profile.matches(&key) {
            self.show_profile_selector();
            return false;
        }

        // SSO Login (L)
        if km.sso_login.matches(&key) {
            self.pending_sso_login = true;
            return false;
        }

        // Toggle theme (Ctrl+L)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            crate::ui::style::theme::toggle_mode();
            return false;
        }

        // Tab-specific actions
        match self.active_tab {
            TAB_ECS => {
                // f = force new deployment
                if km.force_deploy.matches(&key) {
                    self.handle_force_deploy();
                    return false;
                }
            }
            TAB_TASKS => {
                // e = exec into container
                if km.exec.matches(&key) {
                    if !self.session_plugin_installed {
                        self.confirm.show("session-manager-plugin is not installed.\nIt is required for ECS exec and SSM sessions.\n\nInstall it now?");
                        self.pending_action = Some(PendingAction::InstallSessionPlugin);
                        return false;
                    }
                    self.choice_mode = ChoiceMode::ShellSelector;
                    self.choice.show(
                        "Select shell",
                        vec![
                            Choice {
                                key: '1',
                                label: "/bin/sh".to_string(),
                            },
                            Choice {
                                key: '2',
                                label: "/bin/bash".to_string(),
                            },
                            Choice {
                                key: '3',
                                label: "/bin/zsh".to_string(),
                            },
                        ],
                    );
                    return false;
                }
                // x = stop task
                if km.stop_task.matches(&key) {
                    self.handle_stop_task();
                    return false;
                }
                // l = view logs for selected service
                if km.logs.matches(&key) {
                    self.handle_view_logs();
                    return false;
                }
            }
            TAB_SSM => {
                // s = start SSM session
                if km.session.matches(&key) {
                    if !self.session_plugin_installed {
                        self.confirm.show("session-manager-plugin is not installed.\nIt is required for ECS exec and SSM sessions.\n\nInstall it now?");
                        self.pending_action = Some(PendingAction::InstallSessionPlugin);
                        return false;
                    }
                    self.pending_exec = true;
                    return false;
                }
            }
            TAB_LOGS => {
                // f = live tail
                if km.follow.matches(&key) {
                    self.handle_live_tail();
                    return false;
                }
                // i = Insights query
                if km.insights.matches(&key) {
                    self.handle_insights_query();
                    return false;
                }
                // S = export logs
                if km.export.matches(&key) {
                    self.handle_export_logs();
                    return false;
                }
            }
            TAB_RDS => {
                // c = connect
                if key.code == KeyCode::Char('c') {
                    self.handle_rds_connect();
                    return false;
                }
                // d = disconnect
                if key.code == KeyCode::Char('d') {
                    if self.rds_connection.is_some() {
                        self.rds_connection = None;
                        self.rds_tables.set_tables(vec![]);
                        self.query_results.clear();
                        self.kill_ssm_tunnel();
                        self.set_info("Disconnected".to_string());
                    }
                    return false;
                }
                // s = SQL query
                if key.code == KeyCode::Char('s') {
                    if self.rds_connection.is_some() {
                        self.show_sql_query_input();
                    } else {
                        self.set_error("Not connected. Press c to connect.".to_string());
                    }
                    return false;
                }
                // h = SQL history
                if key.code == KeyCode::Char('H') && !self.sql_history.is_empty() {
                    self.show_sql_history();
                    return false;
                }
                // e = SQL modify query (INSERT/UPDATE/DELETE/DDL)
                if km.sql_modify.matches(&key) {
                    if self.rds_connection.is_some() {
                        self.show_sql_modify_input();
                    } else {
                        self.set_error("Not connected. Press c to connect.".to_string());
                    }
                    return false;
                }
                // E = export query results to CSV
                if key.code == KeyCode::Char('E') {
                    self.handle_export_query_results();
                    return false;
                }
                // i = import SQL file
                if key.code == KeyCode::Char('i') && self.rds_connection.is_some() {
                    self.input_mode = InputMode::ImportSql;
                    self.input.show("Import SQL file", "path to .sql file...");
                    return false;
                }
            }
            TAB_S3 => {
                // d = download
                if km.download.matches(&key) {
                    self.handle_s3_download();
                    return false;
                }
                // u = upload
                if km.upload.matches(&key) {
                    self.handle_s3_upload_prompt();
                    return false;
                }
                // x = delete
                if km.delete_object.matches(&key) {
                    self.handle_s3_delete();
                    return false;
                }
                // s = cycle sort
                if km.sort.matches(&key) {
                    if self.active_panel == 0 {
                        self.buckets.cycle_sort();
                    } else {
                        self.objects.cycle_sort();
                    }
                    return false;
                }
            }
            _ => {}
        }

        false
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::Quit => {}
            Action::InputSubmit(value) => {
                match self.input_mode {
                    InputMode::InsightsQuery => {
                        self.last_insights_query = value.clone();
                        self.input_mode = InputMode::None;
                        self.run_insights_query(&value);
                    }
                    InputMode::LogFilter => {
                        self.log_viewer.set_filter(&value);
                        self.input_mode = InputMode::None;
                    }
                    InputMode::PanelFilter => {
                        self.set_active_panel_filter(&value);
                        self.input_mode = InputMode::None;
                    }
                    InputMode::KeywordSearch => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.last_insights_query = format!(
                                "fields @timestamp, @message | filter @message like /{}/ | sort @timestamp desc",
                                value
                            );
                            self.show_insights_query_input();
                        }
                    }
                    InputMode::ExportLogs => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.export_logs_to_file(&value);
                        }
                    }
                    InputMode::S3Upload => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.execute_s3_upload(&value);
                        }
                    }
                    InputMode::CustomDateStart => {
                        self.custom_date_start = value;
                        // Now ask for end date
                        self.input_mode = InputMode::CustomDateEnd;
                        let default_end = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
                        self.input.show_with_value(
                            "End date (UTC)",
                            "YYYY-MM-DD HH:MM",
                            &default_end,
                        );
                    }
                    InputMode::CustomDateEnd => {
                        // Parse both dates
                        let start = parse_datetime(&self.custom_date_start);
                        let end = parse_datetime(&value);
                        self.input_mode = InputMode::None;
                        match (start, end) {
                            (Some(s), Some(e)) => {
                                self.insights_time_range = TimeRange::Absolute { start: s, end: e };
                                self.show_insights_query_input();
                            }
                            _ => {
                                self.err =
                                    Some("Invalid date format. Use YYYY-MM-DD HH:MM".to_string());
                            }
                        }
                    }
                    InputMode::RdsUsername => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.pending_rds_user = value;
                            self.input_mode = InputMode::RdsPassword;
                            self.input.show_password("Password", "enter password...");
                        }
                    }
                    InputMode::RdsPassword => {
                        self.input_mode = InputMode::None;
                        self.pending_rds_password = value;
                        self.input_mode = InputMode::RdsDatabase;
                        let default_db = self
                            .rds_instances
                            .selected()
                            .and_then(|i| i.db_name.clone())
                            .unwrap_or_default();
                        if default_db.is_empty() {
                            self.input
                                .show("Database (optional, Enter to skip)", "database name...");
                        } else {
                            self.input.show_with_value(
                                "Database (optional, Enter to skip)",
                                "database name...",
                                &default_db,
                            );
                        }
                    }
                    InputMode::RdsDatabase => {
                        self.input_mode = InputMode::None;
                        let database = if value.is_empty() { None } else { Some(value) };
                        self.finalize_rds_connection(database);
                    }
                    InputMode::SqlQuery => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.add_to_sql_history(&value);
                            self.last_sql_query = value.clone();
                            self.spawn_execute_query(&value);
                        }
                    }
                    InputMode::SqlModify => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            if !is_modification_query(&value) {
                                self.set_error("Use 's' for SELECT queries.".to_string());
                                return;
                            }
                            self.add_to_sql_history(&value);
                            self.last_sql_modify_query = value.clone();
                            if is_dml_query(&value) {
                                // DML: preview first (transaction + rollback)
                                self.spawn_preview_dml(&value);
                            } else {
                                // DDL: can't preview (implicit commit), confirm directly
                                let truncated = if value.len() > 80 {
                                    format!("{}...", &value[..80])
                                } else {
                                    value.clone()
                                };
                                self.confirm
                                    .show(&format!("Execute modification query?\n\n{}", truncated));
                                self.pending_action =
                                    Some(PendingAction::ExecuteDmlQuery { sql: value });
                            }
                        }
                    }
                    InputMode::ExportQueryResults => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.export_query_results_to_file(&value);
                        }
                    }
                    InputMode::ImportSql => {
                        self.input_mode = InputMode::None;
                        if !value.is_empty() {
                            self.pending_import_path = Some(value);
                            self.pending_import_sql = true;
                        }
                    }
                    InputMode::None => {}
                }
            }
            Action::InputCancel => {
                self.input_mode = InputMode::None;
            }
            Action::SwitchTab(tab) => self.switch_tab(tab),
            Action::Refresh => self.refresh_current_tab(),
            Action::None => {}
        }
    }

    fn execute_pending_action(&mut self) {
        let action = match self.pending_action.take() {
            Some(a) => a,
            None => return,
        };

        // Handle actions that don't need the runner
        if let PendingAction::SaveRdsCredentials {
            profile,
            identifier,
            username,
            password,
            database,
        } = action
        {
            let encoded = credentials::encode_password(&password);
            self.saved_credentials
                .profiles
                .entry(profile)
                .or_default()
                .rds
                .insert(
                    identifier.clone(),
                    credentials::RdsCredential {
                        username,
                        password: encoded,
                        database,
                    },
                );
            match credentials::save(&self.saved_credentials) {
                Ok(()) => {
                    self.credentials_just_saved = true;
                    self.set_info(format!("Credentials saved for {identifier}"));
                }
                Err(e) => {
                    self.set_error(format!("Failed to save: {e}"));
                }
            }
            return;
        }

        if let PendingAction::ExecuteDmlQuery { sql } = action {
            let actual_sql = if is_dml_query(&sql) {
                format!("{sql}; SELECT ROW_COUNT() AS 'Rows affected';")
            } else {
                sql.clone()
            };
            self.last_sql_query = sql;
            self.pending_dml_refresh = true;
            self.spawn_execute_query(&actual_sql);
            return;
        }

        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };

        match action {
            PendingAction::ForceDeploy { cluster, service } => {
                log::info!("force deploy: {cluster}/{service}");
                self.output.clear();
                self.output
                    .append_line(&format!("Force deploying {service}..."));
                match runner.force_new_deployment(&cluster, &service) {
                    Ok(handle) => {
                        self.stream_rx = Some(handle.rx);
                        self.child_pid = handle.child_pid;
                        self.loading = true;
                        self.spinner.start("Deploying...");
                    }
                    Err(e) => {
                        self.set_error(format!("Deploy failed: {e}"));
                    }
                }
            }
            PendingAction::InstallSessionPlugin => {
                self.pending_install_plugin = true;
            }
            PendingAction::StopTask { cluster, task } => {
                log::info!("stop task: {cluster}/{task}");
                thread::spawn(move || match runner.stop_task(&cluster, &task) {
                    Ok(_) => log::info!("task stopped"),
                    Err(e) => log::error!("stop task error: {e}"),
                });
                self.spawn_load_tasks();
            }
            PendingAction::SaveRdsCredentials { .. } => {
                // Handled above before runner extraction
            }
            PendingAction::DeleteS3Object { bucket, key } => {
                log::info!("delete s3 object: s3://{bucket}/{key}");
                self.spinner.start("Deleting...");
                let tx = self.bg_tx.clone();
                thread::spawn(move || match runner.delete_object(&bucket, &key) {
                    Ok(()) => {
                        let _ = tx.send(BgMsg::DeleteObjectDone);
                    }
                    Err(e) => {
                        let _ = tx.send(BgMsg::DeleteObjectError(e));
                    }
                });
            }
            PendingAction::ExecuteDmlQuery { .. } => {
                // Handled above before runner extraction
            }
        }
    }

    fn run_insights_query(&mut self, query: &str) {
        self.add_to_query_history(query);
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let group = match self.log_groups.selected() {
            Some(g) => g.log_group_name.clone(),
            None => {
                self.set_error("No log group selected".to_string());
                return;
            }
        };

        let (start_time, end_time) = self.insights_time_range.to_timestamps();
        let range_label = self.insights_time_range.label();

        log::info!("running Insights query on {group} ({range_label}): {query}");
        self.log_viewer.clear();
        self.log_viewer
            .append_line(&format!("Running query on {group}"));
        self.log_viewer
            .append_line(&format!("Time range: {range_label}"));
        self.log_viewer.append_line(&format!("> {query}"));
        self.log_viewer.append_line("");
        self.spinner.start("Running query...");

        let query = query.to_string();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let start_result = runner.start_insights_query(&group, &query, start_time, end_time);
            match start_result {
                Ok(query_id) => {
                    // Poll for results
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        match runner.get_insights_results(&query_id) {
                            Ok((status, results)) => {
                                let _ = tx.send(BgMsg::InsightsResults {
                                    status: status.clone(),
                                    results: results.clone(),
                                });
                                if status == "Complete"
                                    || status == "Failed"
                                    || status == "Cancelled"
                                {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(BgMsg::InsightsError(e));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::InsightsError(e));
                }
            }
        });
    }

    fn switch_tab(&mut self, tab: usize) {
        self.active_tab = tab;
        self.active_panel = 0;
        self.err = None;

        // Lazy loading on first visit
        match tab {
            TAB_SSM if !self.ssm_visited => {
                self.ssm_visited = true;
                self.spawn_load_instances();
            }
            TAB_LOGS if !self.logs_visited => {
                self.logs_visited = true;
                self.spawn_load_log_groups();
            }
            TAB_RDS if !self.rds_visited => {
                self.rds_visited = true;
                self.spawn_load_rds_instances();
            }
            TAB_S3 if !self.s3_visited => {
                self.s3_visited = true;
                self.spawn_load_buckets();
            }
            _ => {}
        }
    }

    fn refresh_current_tab(&mut self) {
        match self.active_tab {
            TAB_ECS => {
                self.spawn_load_clusters();
                if self.selected_cluster.is_some() {
                    self.spawn_load_services();
                }
            }
            TAB_TASKS => {
                if self.selected_cluster.is_some() && self.selected_service.is_some() {
                    self.spawn_load_tasks();
                }
            }
            TAB_SSM => {
                self.spawn_load_instances();
            }
            TAB_LOGS => {
                self.spawn_load_log_groups();
            }
            TAB_RDS => {
                self.spawn_load_rds_instances();
            }
            TAB_S3 => {
                self.spawn_load_buckets();
                if self.selected_bucket.is_some() {
                    self.spawn_load_objects();
                }
            }
            _ => {}
        }
    }

    fn navigate_up(&mut self) {
        match self.active_tab {
            TAB_ECS => {
                if self.active_panel == 0 {
                    self.clusters.move_up();
                } else {
                    self.services.move_up();
                }
            }
            TAB_TASKS => {
                if self.active_panel == 0 {
                    self.tasks.move_up();
                } else {
                    self.containers.move_up();
                }
            }
            TAB_SSM => {
                self.instances.move_up();
            }
            TAB_LOGS => match self.active_panel {
                0 => self.log_groups.move_up(),
                1 => self.log_streams.move_up(),
                2 => self.log_viewer.move_up(),
                _ => {}
            },
            TAB_RDS => match self.active_panel {
                0 => self.rds_instances.move_up(),
                1 => self.rds_tables.move_up(),
                2 => self.query_results.move_up(),
                _ => {}
            },
            TAB_S3 => {
                if self.active_panel == 0 {
                    self.buckets.move_up();
                } else {
                    self.objects.move_up();
                }
            }
            _ => {}
        }
    }

    fn navigate_down(&mut self) {
        match self.active_tab {
            TAB_ECS => {
                if self.active_panel == 0 {
                    self.clusters.move_down();
                } else {
                    self.services.move_down();
                }
            }
            TAB_TASKS => {
                if self.active_panel == 0 {
                    self.tasks.move_down();
                } else {
                    self.containers.move_down();
                }
            }
            TAB_SSM => {
                self.instances.move_down();
            }
            TAB_LOGS => match self.active_panel {
                0 => self.log_groups.move_down(),
                1 => self.log_streams.move_down(),
                2 => self.log_viewer.move_down(),
                _ => {}
            },
            TAB_RDS => match self.active_panel {
                0 => self.rds_instances.move_down(),
                1 => self.rds_tables.move_down(),
                2 => self.query_results.move_down(),
                _ => {}
            },
            TAB_S3 => {
                if self.active_panel == 0 {
                    self.buckets.move_down();
                } else {
                    self.objects.move_down();
                }
            }
            _ => {}
        }
    }

    fn handle_enter(&mut self) {
        match self.active_tab {
            TAB_ECS => {
                if self.active_panel == 0 {
                    // Select cluster → load services
                    if let Some(cluster) = self.clusters.selected() {
                        self.selected_cluster = Some(cluster.cluster_name.clone());
                        self.active_panel = 1;
                        self.spawn_load_services();
                    }
                } else {
                    // Select service → switch to Tasks tab
                    if let Some(svc) = self.services.selected() {
                        self.selected_service = Some(svc.service_name.clone());
                        self.switch_tab(TAB_TASKS);
                        self.spawn_load_tasks();
                    }
                }
            }
            TAB_TASKS => {
                if self.active_panel == 0 {
                    // Select task → show containers
                    if let Some(task) = self.tasks.selected() {
                        self.containers.set_containers(task.containers.clone());
                        self.active_panel = 1;
                    }
                }
            }
            TAB_LOGS => {
                if self.active_panel == 0 {
                    // Select log group → load streams
                    if self.log_groups.selected().is_some() {
                        self.active_panel = 1;
                        self.spawn_load_log_streams();
                    }
                } else if self.active_panel == 1 {
                    // Select stream → focus on log viewer
                    self.active_panel = 2;
                }
            }
            TAB_RDS => {
                if self.active_panel == 1 {
                    // Select table → SELECT * FROM table
                    if let Some(table) = self.rds_tables.selected().cloned() {
                        if self.rds_connection.is_some() {
                            let query = format!("SELECT * FROM `{table}` LIMIT 100");
                            self.add_to_sql_history(&query);
                            self.last_sql_query = query.clone();
                            self.spawn_execute_query(&query);
                        }
                    }
                }
            }
            TAB_S3 => {
                if self.active_panel == 0 {
                    // Select bucket → load objects
                    if let Some(bucket) = self.buckets.selected() {
                        let name = bucket.name.clone();
                        self.selected_bucket = Some(name.clone());
                        self.objects.bucket_name = name;
                        self.objects.navigate_into("");
                        self.active_panel = 1;
                        self.spawn_load_objects();
                    }
                } else if let Some(item) = self.objects.selected().cloned() {
                    match item {
                        panels::S3ObjectItem::ParentDir => {
                            self.objects.go_up();
                            self.spawn_load_objects();
                        }
                        panels::S3ObjectItem::Prefix(p) => {
                            self.objects.navigate_into(&p);
                            self.spawn_load_objects();
                        }
                        panels::S3ObjectItem::Object(_) => {
                            // No action on Enter for objects
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_back(&mut self) {
        match self.active_tab {
            TAB_ECS if self.active_panel == 1 => {
                self.active_panel = 0;
            }
            TAB_TASKS => {
                self.switch_tab(TAB_ECS);
                self.active_panel = 1;
            }
            TAB_SSM if self.active_panel == 1 => {
                self.active_panel = 0;
            }
            TAB_LOGS if self.active_panel == 2 => {
                self.active_panel = 1;
            }
            TAB_LOGS if self.active_panel == 1 => {
                self.active_panel = 0;
            }
            TAB_S3 if self.active_panel == 1 => {
                if self.objects.current_prefix.is_empty() {
                    self.active_panel = 0;
                } else {
                    self.objects.go_up();
                    self.spawn_load_objects();
                }
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        let col = mouse.column;
        let row = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Tab bar click
                if row == 0 {
                    if let Some(tab) = tab_index_at(col, self.hit_tab_bar.width) {
                        self.switch_tab(tab);
                    }
                    return;
                }

                // Panel focus on click
                if self.is_in_rect(col, row, self.hit_top_panel) {
                    self.active_panel = 0;
                } else if self.is_in_rect(col, row, self.hit_bottom_panel) {
                    self.active_panel = 1;
                } else if self.is_in_rect(col, row, self.hit_right_panel)
                    && (self.layout.mode == LayoutMode::Vertical
                        || self.active_tab == TAB_LOGS
                        || self.active_tab == TAB_RDS)
                {
                    self.active_panel = 2;
                }
            }
            MouseEventKind::ScrollUp => {
                if self.is_in_rect(col, row, self.hit_top_panel) {
                    self.active_panel = 0;
                    self.navigate_up();
                } else if self.is_in_rect(col, row, self.hit_bottom_panel) {
                    self.active_panel = 1;
                    self.navigate_up();
                } else if self.is_in_rect(col, row, self.hit_right_panel)
                    && (self.layout.mode == LayoutMode::Vertical
                        || self.active_tab == TAB_LOGS
                        || self.active_tab == TAB_RDS)
                {
                    self.active_panel = 2;
                    self.navigate_up();
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_in_rect(col, row, self.hit_top_panel) {
                    self.active_panel = 0;
                    self.navigate_down();
                } else if self.is_in_rect(col, row, self.hit_bottom_panel) {
                    self.active_panel = 1;
                    self.navigate_down();
                } else if self.is_in_rect(col, row, self.hit_right_panel)
                    && (self.layout.mode == LayoutMode::Vertical
                        || self.active_tab == TAB_LOGS
                        || self.active_tab == TAB_RDS)
                {
                    self.active_panel = 2;
                    self.navigate_down();
                }
            }
            _ => {}
        }
    }

    fn is_in_rect(&self, col: u16, row: u16, rect: Rect) -> bool {
        col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
    }

    /// Recolor the 1-cell-thick border of `area` without touching its interior.
    fn recolor_border(buf: &mut ratatui::buffer::Buffer, area: Rect, color: ratatui::style::Color) {
        if area.width < 2 || area.height < 2 {
            return;
        }
        let x0 = area.x;
        let y0 = area.y;
        let x1 = area.x + area.width - 1;
        let y1 = area.y + area.height - 1;
        for x in x0..=x1 {
            if let Some(cell) = buf.cell_mut((x, y0)) {
                cell.set_fg(color);
            }
            if let Some(cell) = buf.cell_mut((x, y1)) {
                cell.set_fg(color);
            }
        }
        for y in y0..=y1 {
            if let Some(cell) = buf.cell_mut((x0, y)) {
                cell.set_fg(color);
            }
            if let Some(cell) = buf.cell_mut((x1, y)) {
                cell.set_fg(color);
            }
        }
    }

    fn render(&mut self, f: &mut ratatui::Frame) {
        let size = f.area();
        self.layout = compute_layout(size.width, size.height, self.layout_mode_override);

        // Layout: tab bar (1 line) + content + status bar (1 line)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(TAB_BAR_H),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(size);

        let tab_area = chunks[0];
        let content_area = chunks[1];
        let status_area = chunks[2];

        self.hit_tab_bar = tab_area;

        // Tab bar
        render_tab_bar(self.active_tab, tab_area, f.buffer_mut());

        // Clamp active_panel in case the allowed range changed (layout toggled).
        let max_panels = self.max_panels();
        if self.active_panel >= max_panels {
            self.active_panel = max_panels.saturating_sub(1);
        }

        // Vertical layout: all panels stacked with the active one taking the remaining
        // space and the others collapsed to their title bar (lazygit portrait style).
        if self.layout.mode == LayoutMode::Vertical {
            let rects = self.vertical_panel_rects(content_area);
            self.hit_top_panel = *rects.first().unwrap_or(&Rect::default());
            self.hit_bottom_panel = *rects.get(1).unwrap_or(&Rect::default());
            self.hit_right_panel = *rects.get(2).unwrap_or(&Rect::default());
            self.render_vertical_content(content_area, f.buffer_mut());
            self.render_status_bar(status_area, f.buffer_mut());
            self.render_overlays(size, f);
            return;
        }

        // Content: left (50%) + right (50%)
        let h_ratio = if self.active_tab == TAB_LOGS || self.active_tab == TAB_RDS {
            25
        } else {
            self.split_horizontal
        };
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(h_ratio),
                Constraint::Percentage(100 - h_ratio),
            ])
            .split(content_area);

        let left_area = content_chunks[0];
        let right_area = content_chunks[1];

        // Left: two stacked panels (70/30 when both have content)
        let left_panels = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(self.split_vertical),
                Constraint::Percentage(100 - self.split_vertical),
            ])
            .split(left_area);

        let top_panel_area = left_panels[0];
        let bottom_panel_area = left_panels[1];

        // Store hit zones for mouse handling
        self.hit_top_panel = top_panel_area;
        self.hit_bottom_panel = bottom_panel_area;
        self.hit_right_panel = right_area;

        // Render panels based on active tab
        match self.active_tab {
            TAB_ECS => {
                self.clusters.render(
                    top_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 0,
                    self.loading_clusters,
                );
                self.services.render(
                    bottom_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 1,
                    self.loading_services,
                );
                // Update detail based on focused panel
                if self.active_panel == 1 {
                    if let Some(svc) = self.services.selected() {
                        self.detail.set_lines(format_service_detail(svc));
                    }
                } else if let Some(cluster) = self.clusters.selected() {
                    self.detail.set_lines(format_cluster_detail(cluster));
                }
                self.detail.render(right_area, f.buffer_mut(), false);
            }
            TAB_TASKS => {
                self.tasks.render(
                    top_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 0,
                    self.loading_tasks,
                );
                self.containers.render(
                    bottom_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 1,
                    false,
                );
                if self.terminal.is_active() {
                    self.terminal.render(right_area, f.buffer_mut());
                } else {
                    // Update detail based on focused panel
                    if self.active_panel == 1 {
                        if let Some(container) = self.containers.selected() {
                            self.detail.set_lines(format_container_detail(container));
                        }
                    } else if let Some(task) = self.tasks.selected() {
                        self.detail.set_lines(format_task_detail(task));
                    }
                    self.detail.render(right_area, f.buffer_mut(), false);
                }
            }
            TAB_SSM => {
                self.instances
                    .render(top_panel_area, f.buffer_mut(), true, self.loading_instances);
                let block = Block::default()
                    .title(" Sessions ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::color_border()));
                block.render(bottom_panel_area, f.buffer_mut());
                if let Some(inst) = self.instances.selected() {
                    self.detail.set_lines(format_instance_detail(inst));
                }
                self.detail.render(right_area, f.buffer_mut(), false);
            }
            TAB_LOGS => {
                self.log_groups.render(
                    top_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 0,
                    self.loading_log_groups,
                );
                self.log_streams.render(
                    bottom_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 1,
                    self.loading_log_streams,
                );

                // Right side: split into log list (top) + log detail (bottom)
                let right_panels = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(self.split_vertical),
                        Constraint::Percentage(100 - self.split_vertical),
                    ])
                    .split(right_area);

                self.log_viewer
                    .render(right_panels[0], f.buffer_mut(), self.active_panel == 2);

                let selected_log = self.log_viewer.selected_line().unwrap_or("").to_string();
                panels::render_log_detail(
                    &selected_log,
                    right_panels[1],
                    f.buffer_mut(),
                    self.active_panel == 2,
                );
            }
            TAB_RDS => {
                self.rds_instances.render(
                    top_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 0,
                    self.loading_rds_instances,
                );
                self.rds_tables.render(
                    bottom_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 1,
                    self.loading_rds_tables,
                );
                if self.rds_connection.is_some() {
                    self.query_results.render(
                        right_area,
                        f.buffer_mut(),
                        self.active_panel == 2,
                        self.loading_query,
                    );
                } else {
                    if let Some(inst) = self.rds_instances.selected() {
                        self.detail.set_lines(format_rds_instance_detail(inst));
                    }
                    self.detail.render(right_area, f.buffer_mut(), false);
                }
            }
            TAB_S3 => {
                self.buckets.render(
                    top_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 0,
                    self.loading_buckets,
                );
                self.objects.render(
                    bottom_panel_area,
                    f.buffer_mut(),
                    self.active_panel == 1,
                    self.loading_objects,
                );
                // Update detail panel based on focus
                if self.active_panel == 1 {
                    match self.objects.selected() {
                        Some(panels::S3ObjectItem::Object(obj)) => {
                            let bucket = self.selected_bucket.as_deref().unwrap_or("?").to_string();
                            self.detail.set_lines(format_object_detail(obj, &bucket));
                        }
                        Some(panels::S3ObjectItem::Prefix(p)) => {
                            let bucket = self.selected_bucket.as_deref().unwrap_or("?").to_string();
                            self.detail.set_lines(vec![
                                format!("Prefix: {p}"),
                                String::new(),
                                format!("URI: s3://{bucket}/{p}"),
                            ]);
                        }
                        _ => {
                            self.detail.clear();
                        }
                    }
                } else if let Some(bucket) = self.buckets.selected() {
                    self.detail.set_lines(format_bucket_detail(bucket));
                }
                self.detail.render(right_area, f.buffer_mut(), false);
            }
            _ => {}
        }

        // Overlay: recolor borders when resize mode is active (horizontal layout only).
        if self.resize_mode == ResizeMode::Active {
            let accent = theme::color_primary();
            Self::recolor_border(f.buffer_mut(), top_panel_area, accent);
            Self::recolor_border(f.buffer_mut(), bottom_panel_area, accent);
            Self::recolor_border(f.buffer_mut(), right_area, accent);
        }

        self.render_status_bar(status_area, f.buffer_mut());
        self.render_overlays(size, f);
    }

    /// Number of tab-cyclable panels for the current tab in the current layout mode.
    fn max_panels(&self) -> usize {
        if self.layout.mode == LayoutMode::Vertical {
            match self.active_tab {
                TAB_SSM => 2, // Instances + Detail (skip empty Sessions block)
                _ => 3,       // top + bottom + detail/right
            }
        } else {
            match self.active_tab {
                TAB_LOGS | TAB_RDS => 3,
                _ => 2,
            }
        }
    }

    /// Compute stacked rects for the current tab's panels (lazygit-style).
    /// The active panel takes remaining space; inactives collapse to `COLLAPSED_PANEL_H` rows.
    fn vertical_panel_rects(&self, area: Rect) -> Vec<Rect> {
        let count = self.max_panels();
        let constraints: Vec<Constraint> = (0..count)
            .map(|i| {
                if i == self.active_panel {
                    Constraint::Fill(1)
                } else {
                    Constraint::Length(crate::ui::layout::COLLAPSED_PANEL_H)
                }
            })
            .collect();
        Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    /// Render all panels of the current tab stacked vertically (lazygit portrait style).
    fn render_vertical_content(&mut self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let rects = self.vertical_panel_rects(area);
        let active = self.active_panel;

        match self.active_tab {
            TAB_ECS => {
                self.clusters
                    .render(rects[0], buf, active == 0, self.loading_clusters);
                self.services
                    .render(rects[1], buf, active == 1, self.loading_services);
                if active == 1 {
                    if let Some(svc) = self.services.selected() {
                        self.detail.set_lines(format_service_detail(svc));
                    }
                } else if let Some(cluster) = self.clusters.selected() {
                    self.detail.set_lines(format_cluster_detail(cluster));
                }
                self.detail.render(rects[2], buf, active == 2);
            }
            TAB_TASKS => {
                self.tasks
                    .render(rects[0], buf, active == 0, self.loading_tasks);
                self.containers.render(rects[1], buf, active == 1, false);
                if self.terminal.is_active() {
                    self.terminal.render(rects[2], buf);
                } else {
                    if active == 1 {
                        if let Some(container) = self.containers.selected() {
                            self.detail.set_lines(format_container_detail(container));
                        }
                    } else if let Some(task) = self.tasks.selected() {
                        self.detail.set_lines(format_task_detail(task));
                    }
                    self.detail.render(rects[2], buf, active == 2);
                }
            }
            TAB_SSM => {
                self.instances
                    .render(rects[0], buf, active == 0, self.loading_instances);
                if let Some(inst) = self.instances.selected() {
                    self.detail.set_lines(format_instance_detail(inst));
                }
                self.detail.render(rects[1], buf, active == 1);
            }
            TAB_LOGS => {
                self.log_groups
                    .render(rects[0], buf, active == 0, self.loading_log_groups);
                self.log_streams
                    .render(rects[1], buf, active == 1, self.loading_log_streams);
                // Third slot: log viewer. When active, split it into viewer + detail.
                if active == 2 && rects[2].height > crate::ui::layout::COLLAPSED_PANEL_H * 2 {
                    let inner = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Percentage(self.split_vertical),
                            Constraint::Percentage(100 - self.split_vertical),
                        ])
                        .split(rects[2]);
                    self.log_viewer.render(inner[0], buf, true);
                    let selected_log = self.log_viewer.selected_line().unwrap_or("").to_string();
                    panels::render_log_detail(&selected_log, inner[1], buf, true);
                } else {
                    self.log_viewer.render(rects[2], buf, active == 2);
                }
            }
            TAB_RDS => {
                self.rds_instances
                    .render(rects[0], buf, active == 0, self.loading_rds_instances);
                self.rds_tables
                    .render(rects[1], buf, active == 1, self.loading_rds_tables);
                if self.rds_connection.is_some() {
                    self.query_results
                        .render(rects[2], buf, active == 2, self.loading_query);
                } else {
                    if let Some(inst) = self.rds_instances.selected() {
                        self.detail.set_lines(format_rds_instance_detail(inst));
                    }
                    self.detail.render(rects[2], buf, active == 2);
                }
            }
            TAB_S3 => {
                self.buckets
                    .render(rects[0], buf, active == 0, self.loading_buckets);
                self.objects
                    .render(rects[1], buf, active == 1, self.loading_objects);
                if active == 1 {
                    match self.objects.selected() {
                        Some(panels::S3ObjectItem::Object(obj)) => {
                            let bucket = self.selected_bucket.as_deref().unwrap_or("?").to_string();
                            self.detail.set_lines(format_object_detail(obj, &bucket));
                        }
                        Some(panels::S3ObjectItem::Prefix(p)) => {
                            let bucket = self.selected_bucket.as_deref().unwrap_or("?").to_string();
                            self.detail.set_lines(vec![
                                format!("Prefix: {p}"),
                                String::new(),
                                format!("URI: s3://{bucket}/{p}"),
                            ]);
                        }
                        _ => {
                            self.detail.clear();
                        }
                    }
                } else if let Some(bucket) = self.buckets.selected() {
                    self.detail.set_lines(format_bucket_detail(bucket));
                }
                self.detail.render(rects[2], buf, active == 2);
            }
            _ => {}
        }

        if self.resize_mode == ResizeMode::Active {
            // Recolor only the active panel's border (no splits to resize in vertical mode).
            Self::recolor_border(buf, rects[active], theme::color_primary());
        }
    }

    fn render_status_bar(&self, status_area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let mut sb = StatusBar::new();
        sb.set_width(status_area.width);
        let hints = if self.resize_mode == ResizeMode::Active {
            vec![
                Hint {
                    key: "←→".to_string(),
                    desc: "h-split".to_string(),
                },
                Hint {
                    key: "↑↓".to_string(),
                    desc: "v-split".to_string(),
                },
                Hint {
                    key: "Esc".to_string(),
                    desc: "done".to_string(),
                },
            ]
        } else if self.layout.mode == LayoutMode::Vertical {
            let mut h = default_hints(self.active_tab);
            h.insert(
                0,
                Hint {
                    key: format!("{}/{}", self.active_panel + 1, self.max_panels()),
                    desc: "panel".to_string(),
                },
            );
            h
        } else {
            default_hints(self.active_tab)
        };
        sb.set_hints(hints);

        // AWS info on the right
        let profile_name = self.active_profile.as_deref().unwrap_or("no profile");
        let aws_status = format!("{} | {}", profile_name, self.active_region);
        if let Some(ref id) = self.caller_identity {
            sb.set_aws_info(&format!("{aws_status} | {}", id.account));
        } else {
            sb.set_aws_info(&aws_status);
        }

        if self.spinner.is_active() {
            sb.set_loading(&self.spinner.view());
        }

        sb.render(status_area, buf);

        // Status messages overlay
        if let Some(ref info) = self.info {
            let info_text = format!(" {info} ");
            let style = styles::success_style();
            let x = status_area.x + 1;
            let y = status_area.y;
            buf.set_string(x, y, &info_text, style);
        } else if let Some(ref err) = self.err {
            let err_text = format!(" Error: {err} ");
            let style = styles::error_style();
            let x = status_area.x + 1;
            let y = status_area.y;
            buf.set_string(x, y, &err_text, style);
        }
    }

    fn render_overlays(&mut self, size: Rect, f: &mut ratatui::Frame) {
        if self.help.is_visible() {
            let popup_area = centered_rect(60, 80, size);
            self.help.render(popup_area, f.buffer_mut());
        }

        if self.confirm.is_visible() {
            let popup_area = centered_rect(45, 25, size);
            Clear.render(popup_area, f.buffer_mut());
            let block = Block::default()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(theme::color_danger())
                        .add_modifier(Modifier::BOLD),
                );
            let inner = block.inner(popup_area);
            block.render(popup_area, f.buffer_mut());
            let text = Paragraph::new(self.confirm.view())
                .style(Style::default().fg(theme::color_bright()))
                .wrap(Wrap { trim: false });
            text.render(inner, f.buffer_mut());
        }

        if self.choice.is_visible() {
            let popup_area = centered_rect(40, 30, size);
            Clear.render(popup_area, f.buffer_mut());
            let block = Block::default()
                .title(" Choose ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::color_primary()));
            let inner = block.inner(popup_area);
            block.render(popup_area, f.buffer_mut());
            let text = Paragraph::new(self.choice.view())
                .style(Style::default().fg(theme::color_text()))
                .wrap(Wrap { trim: false });
            text.render(inner, f.buffer_mut());
        }

        if self.input.is_visible() {
            let popup_area = centered_rect(70, 20, size);
            self.input.render(popup_area, f.buffer_mut());
        }
    }

    // --- Background thread spawners ---

    // --- Action handlers ---

    fn handle_force_deploy(&mut self) {
        let cluster = match &self.selected_cluster {
            Some(c) => c.clone(),
            None => {
                self.set_error("No cluster selected".to_string());
                return;
            }
        };
        let service = match self.services.selected() {
            Some(s) => s.service_name.clone(),
            None => {
                self.set_error("No service selected".to_string());
                return;
            }
        };
        self.confirm
            .show(&format!("Force new deployment for {service}?"));
        self.pending_action = Some(PendingAction::ForceDeploy { cluster, service });
    }

    fn handle_stop_task(&mut self) {
        let cluster = match &self.selected_cluster {
            Some(c) => c.clone(),
            None => {
                self.set_error("No cluster selected".to_string());
                return;
            }
        };
        let task = match self.tasks.selected() {
            Some(t) => t.task_arn.clone(),
            None => {
                self.set_error("No task selected".to_string());
                return;
            }
        };
        let short_id = task.rsplit('/').next().unwrap_or(&task);
        self.confirm.show(&format!("Stop task {short_id}?"));
        self.pending_action = Some(PendingAction::StopTask { cluster, task });
    }

    fn get_active_panel_filter(&self) -> String {
        match self.active_tab {
            TAB_ECS => {
                if self.active_panel == 0 {
                    self.clusters.filter.clone()
                } else {
                    self.services.filter.clone()
                }
            }
            TAB_TASKS => {
                if self.active_panel == 0 {
                    self.tasks.filter.clone()
                } else {
                    self.containers.filter.clone()
                }
            }
            TAB_SSM => self.instances.filter.clone(),
            TAB_LOGS => {
                if self.active_panel == 0 {
                    self.log_groups.filter.clone()
                } else if self.active_panel == 1 {
                    self.log_streams.filter.clone()
                } else {
                    self.log_viewer.filter.clone()
                }
            }
            TAB_RDS => match self.active_panel {
                0 => self.rds_instances.filter.clone(),
                1 => self.rds_tables.filter.clone(),
                2 => self.query_results.filter.clone(),
                _ => String::new(),
            },
            TAB_S3 => {
                if self.active_panel == 0 {
                    self.buckets.filter.clone()
                } else {
                    self.objects.filter.clone()
                }
            }
            _ => String::new(),
        }
    }

    fn set_active_panel_filter(&mut self, filter: &str) {
        match self.active_tab {
            TAB_ECS => {
                if self.active_panel == 0 {
                    self.clusters.set_filter(filter);
                } else {
                    self.services.set_filter(filter);
                }
            }
            TAB_TASKS => {
                if self.active_panel == 0 {
                    self.tasks.set_filter(filter);
                } else {
                    self.containers.set_filter(filter);
                }
            }
            TAB_SSM => self.instances.set_filter(filter),
            TAB_LOGS => {
                if self.active_panel == 0 {
                    self.log_groups.set_filter(filter);
                } else if self.active_panel == 1 {
                    self.log_streams.set_filter(filter);
                } else {
                    self.log_viewer.set_filter(filter);
                }
            }
            TAB_RDS => match self.active_panel {
                0 => self.rds_instances.set_filter(filter),
                1 => self.rds_tables.set_filter(filter),
                2 => self.query_results.set_filter(filter),
                _ => {}
            },
            TAB_S3 => {
                if self.active_panel == 0 {
                    self.buckets.set_filter(filter);
                } else {
                    self.objects.set_filter(filter);
                }
            }
            _ => {}
        }
    }

    /// Returns true if a filter was cleared.
    fn clear_active_panel_filter(&mut self) -> bool {
        let current = self.get_active_panel_filter();
        if !current.is_empty() {
            self.set_active_panel_filter("");
            true
        } else {
            false
        }
    }

    /// Checks if an error message indicates expired/invalid credentials.
    /// If so, shows a confirm dialog offering SSO re-login.
    fn set_error(&mut self, msg: String) {
        self.err = Some(msg);
        self.info = None;
        self.msg_time = Some(std::time::Instant::now());
    }

    fn set_info(&mut self, msg: String) {
        self.info = Some(msg);
        self.err = None;
        self.msg_time = Some(std::time::Instant::now());
    }

    fn handle_aws_error(&mut self, error: &str) {
        // Only treat actual token/session expiry as auth errors.
        // AccessDenied and ForbiddenException are permission errors (e.g. S3-only
        // role calling ECS), NOT expired credentials.
        let is_auth_error = error.contains("ExpiredToken")
            || error.contains("InvalidIdentityToken")
            || error.contains("The SSO session")
            || error.contains("Token has expired")
            || error.contains("UnrecognizedClientException");

        if is_auth_error && self.active_profile.is_some() {
            let profile = self.active_profile.as_deref().unwrap_or("?");
            self.set_error(format!(
                "Credentials expired for {profile}. Press L to re-login."
            ));
        } else {
            self.set_error(error.to_string());
        }
    }

    fn handle_yank(&mut self) {
        let text = match self.active_tab {
            TAB_ECS => {
                if self.active_panel == 1 {
                    self.services.selected().map(|s| s.service_arn.clone())
                } else {
                    self.clusters.selected().map(|c| c.cluster_arn.clone())
                }
            }
            TAB_TASKS => {
                if self.active_panel == 1 {
                    // Copy container detail (image)
                    self.containers.selected().map(|c| c.image.clone())
                } else {
                    self.tasks.selected().map(|t| t.task_arn.clone())
                }
            }
            TAB_SSM => self.instances.selected().map(|i| i.id.clone()),
            TAB_RDS => match self.active_panel {
                1 => self.rds_tables.selected().cloned(),
                2 => self.query_results.selected_line(),
                _ => self
                    .rds_instances
                    .selected()
                    .map(|i| i.db_instance_arn.clone()),
            },
            TAB_LOGS => {
                if self.active_panel == 2 {
                    // Copy the full selected log line
                    self.log_viewer.selected_line().map(|s| s.to_string())
                } else if self.active_panel == 1 {
                    self.log_streams
                        .selected()
                        .map(|s| s.log_stream_name.clone())
                } else {
                    self.log_groups.selected().map(|g| g.log_group_name.clone())
                }
            }
            TAB_S3 => {
                if self.active_panel == 0 {
                    self.buckets
                        .selected()
                        .map(|b| format!("arn:aws:s3:::{}", b.name))
                } else {
                    match self.objects.selected() {
                        Some(panels::S3ObjectItem::Object(obj)) => {
                            let bucket = self.selected_bucket.as_deref().unwrap_or("?");
                            Some(format!("s3://{}/{}", bucket, obj.key))
                        }
                        Some(panels::S3ObjectItem::Prefix(p)) => {
                            let bucket = self.selected_bucket.as_deref().unwrap_or("?");
                            Some(format!("s3://{}/{}", bucket, p))
                        }
                        _ => None,
                    }
                }
            }
            _ => None,
        };

        if let Some(text) = text {
            match copy_to_clipboard(&text) {
                Ok(()) => {
                    let preview = if text.len() > 60 {
                        format!("{}...", &text[..60])
                    } else {
                        text
                    };
                    self.set_info(format!("Copied: {preview}"));
                    self.err = None;
                }
                Err(e) => {
                    self.set_error(format!("Copy failed: {e}"));
                }
            }
        }
    }

    fn handle_view_logs(&mut self) {
        // Switch to Logs tab and try to find matching log group
        if let Some(svc) = &self.selected_service {
            let prefix = format!("/ecs/{svc}");
            log::info!("switching to logs for prefix: {prefix}");
            self.switch_tab(TAB_LOGS);
            // Trigger load of log groups (will be filtered later)
            self.spawn_load_log_groups();
        }
    }

    fn handle_live_tail(&mut self) {
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let group = match self.log_groups.selected() {
            Some(g) => g.log_group_name.clone(),
            None => {
                self.set_error("No log group selected".to_string());
                return;
            }
        };

        log::info!("starting live tail for {group}");
        self.log_viewer.clear();
        self.log_viewer.follow = true;
        self.log_viewer
            .append_line(&format!("-- tailing {group} --"));

        match runner.tail_logs(&group, "5m") {
            Ok(handle) => {
                self.stream_rx = Some(handle.rx);
                self.child_pid = handle.child_pid;
                self.loading = true;
                self.live_tail_active = true;
                self.active_panel = 2; // Focus on log viewer
                self.spinner.start("Live tail...");
            }
            Err(e) => {
                self.set_error(format!("Failed to tail logs: {e}"));
            }
        }
    }

    fn handle_insights_query(&mut self) {
        if self.log_groups.selected().is_none() {
            self.set_error("No log group selected".to_string());
            return;
        }
        // Go directly to query input, user can press 't' to change time range
        self.show_insights_query_input();
    }

    fn show_time_range_selector(&mut self) {
        let current = self.insights_time_range.label();
        self.choice_mode = ChoiceMode::TimeRangeSelector;
        self.choice.show(
            &format!("Time range (current: {current})"),
            vec![
                Choice {
                    key: '1',
                    label: "15 minutes".to_string(),
                },
                Choice {
                    key: '2',
                    label: "1 hour".to_string(),
                },
                Choice {
                    key: '3',
                    label: "6 hours".to_string(),
                },
                Choice {
                    key: '4',
                    label: "24 hours".to_string(),
                },
                Choice {
                    key: '5',
                    label: "48 hours".to_string(),
                },
                Choice {
                    key: '6',
                    label: "7 days".to_string(),
                },
                Choice {
                    key: '7',
                    label: "Custom date range...".to_string(),
                },
            ],
        );
    }

    fn handle_time_range_choice(&mut self, c: char) {
        match c {
            '1' => self.insights_time_range = TimeRange::Relative(900),
            '2' => self.insights_time_range = TimeRange::Relative(3600),
            '3' => self.insights_time_range = TimeRange::Relative(21600),
            '4' => self.insights_time_range = TimeRange::Relative(86400),
            '5' => self.insights_time_range = TimeRange::Relative(172800),
            '6' => self.insights_time_range = TimeRange::Relative(604800),
            '7' => {
                self.input_mode = InputMode::CustomDateStart;
                let default_start = chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::hours(1))
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default();
                self.input
                    .show_with_value("Start date (UTC)", "YYYY-MM-DD HH:MM", &default_start);
                return;
            }
            _ => return,
        }
        self.show_insights_query_input();
    }

    fn show_insights_query_input(&mut self) {
        self.input_mode = InputMode::InsightsQuery;
        let range_label = self.insights_time_range.label();
        let history_hint = if self.query_history.is_empty() {
            String::new()
        } else {
            format!("  [Ctrl+H: history ({})]", self.query_history.len())
        };
        self.input.show_with_value(
            &format!(
                "Insights Query  [Ctrl+E: templates]  [Ctrl+T: {}]{}",
                range_label, history_hint
            ),
            "Enter query...",
            &self.last_insights_query.clone(),
        );
    }

    fn show_query_templates(&mut self) {
        self.choice_mode = ChoiceMode::QueryTemplate;
        self.choice.show(
            "Query Templates",
            vec![
                Choice {
                    key: '1',
                    label: "All logs (default)".to_string(),
                },
                Choice {
                    key: '2',
                    label: "Filter ERROR".to_string(),
                },
                Choice {
                    key: '3',
                    label: "Filter WARN".to_string(),
                },
                Choice {
                    key: '4',
                    label: "Filter Exception/Stacktrace".to_string(),
                },
                Choice {
                    key: '5',
                    label: "Count by log level".to_string(),
                },
                Choice {
                    key: '6',
                    label: "Top 20 error messages".to_string(),
                },
                Choice {
                    key: '7',
                    label: "Search keyword...".to_string(),
                },
                Choice {
                    key: '8',
                    label: "Latency / duration stats".to_string(),
                },
                Choice {
                    key: '9',
                    label: "Last 200 logs".to_string(),
                },
            ],
        );
    }

    fn show_query_history(&mut self) {
        let choices: Vec<Choice> = self
            .query_history
            .iter()
            .enumerate()
            .take(9)
            .map(|(i, q)| {
                let key = (b'1' + i as u8) as char;
                let label = if q.len() > 60 {
                    format!("{}...", &q[..60])
                } else {
                    q.clone()
                };
                Choice { key, label }
            })
            .collect();
        self.choice_mode = ChoiceMode::QueryHistory;
        self.choice.show("Recent Queries", choices);
    }

    fn handle_export_logs(&mut self) {
        let lines = self.log_viewer.visible_lines();
        if lines.is_empty() {
            self.set_error("No logs to export".to_string());
            return;
        }
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S");
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let default_path = format!("{home}/lazy-aws-logs-{timestamp}.txt");
        self.input_mode = InputMode::ExportLogs;
        self.input.show_with_value(
            &format!("Export {} logs to file (.txt/.json/.csv)", lines.len()),
            "file path...",
            &default_path,
        );
    }

    fn export_logs_to_file(&mut self, path: &str) {
        use std::io::Write;

        let lines: Vec<String> = self
            .log_viewer
            .visible_lines()
            .iter()
            .map(|s| s.to_string())
            .collect();

        if lines.is_empty() {
            self.set_error("No logs to export".to_string());
            return;
        }

        // Expand ~ to home dir
        let expanded = if path.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            path.replacen('~', &home, 1)
        } else {
            path.to_string()
        };

        let content = if expanded.ends_with(".json") {
            // JSON array of strings
            match serde_json::to_string_pretty(&lines) {
                Ok(json) => json,
                Err(e) => {
                    self.set_error(format!("JSON error: {e}"));
                    return;
                }
            }
        } else if expanded.ends_with(".csv") {
            // CSV with header
            let mut csv = String::from("log\n");
            for line in &lines {
                // Quote and escape double quotes
                csv.push('"');
                csv.push_str(&line.replace('"', "\"\""));
                csv.push('"');
                csv.push('\n');
            }
            csv
        } else {
            // Plain text (default)
            lines.join("\n") + "\n"
        };

        match std::fs::File::create(&expanded) {
            Ok(mut file) => match file.write_all(content.as_bytes()) {
                Ok(()) => {
                    self.set_info(format!("Exported {} lines to {}", lines.len(), path));
                    log::info!("exported {} lines to {expanded}", lines.len());
                }
                Err(e) => {
                    self.set_error(format!("Write error: {e}"));
                }
            },
            Err(e) => {
                self.set_error(format!("Cannot create file: {e}"));
            }
        }
    }

    fn handle_export_query_results(&mut self) {
        if self.query_results.columns.is_empty() {
            self.set_error("No query results to export".to_string());
            return;
        }
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S");
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let default_path = format!("{home}/lazy-aws-query-{timestamp}.csv");
        self.input_mode = InputMode::ExportQueryResults;
        self.input.show_with_value(
            &format!("Export {} rows to CSV", self.query_results.rows.len()),
            "file path...",
            &default_path,
        );
    }

    fn export_query_results_to_file(&mut self, path: &str) {
        use std::io::Write;

        if self.query_results.columns.is_empty() {
            self.set_error("No query results to export".to_string());
            return;
        }

        let expanded = if path.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            path.replacen('~', &home, 1)
        } else {
            path.to_string()
        };

        let mut csv = String::new();

        // Header row
        let header: Vec<String> = self
            .query_results
            .columns
            .iter()
            .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
            .collect();
        csv.push_str(&header.join(","));
        csv.push('\n');

        // Data rows
        for row in &self.query_results.rows {
            let cells: Vec<String> = row
                .iter()
                .map(|cell| format!("\"{}\"", cell.replace('"', "\"\"")))
                .collect();
            csv.push_str(&cells.join(","));
            csv.push('\n');
        }

        match std::fs::File::create(&expanded) {
            Ok(mut file) => match file.write_all(csv.as_bytes()) {
                Ok(()) => {
                    self.set_info(format!(
                        "Exported {} rows to {}",
                        self.query_results.rows.len(),
                        path
                    ));
                    log::info!(
                        "exported {} rows to {expanded}",
                        self.query_results.rows.len()
                    );
                }
                Err(e) => {
                    self.set_error(format!("Write error: {e}"));
                }
            },
            Err(e) => {
                self.set_error(format!("Cannot create file: {e}"));
            }
        }
    }

    fn run_import_sql_interactive(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let path = match self.pending_import_path.take() {
            Some(p) => p,
            None => return Ok(()),
        };
        let conn = match &self.rds_connection {
            Some(c) => c,
            None => {
                self.set_error("Not connected".to_string());
                return Ok(());
            }
        };

        let expanded = if path.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            path.replacen('~', &home, 1)
        } else {
            path.clone()
        };

        if !std::path::Path::new(&expanded).exists() {
            self.set_error(format!("File not found: {path}"));
            return Ok(());
        }

        let mut mysql_cmd = format!(
            "mysql -h {} -P {} -u {} --password='{}' ",
            conn.host, conn.port, conn.user, conn.password
        );
        if let Some(ref db) = conn.database {
            mysql_cmd.push_str(db);
            mysql_cmd.push(' ');
        }
        mysql_cmd.push_str(&format!("< '{expanded}'"));

        log::info!("import SQL: mysql ... < {expanded}");

        // Suspend TUI
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )?;

        println!("Importing {path}...\n");

        let status = std::process::Command::new("sh")
            .args(["-c", &mysql_cmd])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("\nImport completed successfully.");
            }
            Ok(s) => {
                println!("\nImport failed (exit {:?})", s.code());
                self.set_error(format!("Import failed (exit {:?})", s.code()));
            }
            Err(e) => {
                println!("\nImport error: {e}");
                self.set_error(format!("Import error: {e}"));
            }
        }

        println!("\nPress Enter to return to lazy-aws...");
        let _ = std::io::stdin().read_line(&mut String::new());

        // Resume TUI
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        terminal.clear()?;

        // Reload tables after import
        self.spawn_load_rds_tables();

        Ok(())
    }

    fn add_to_query_history(&mut self, query: &str) {
        // Remove duplicates
        self.query_history.retain(|q| q != query);
        // Add to front
        self.query_history.insert(0, query.to_string());
        // Keep max 10
        self.query_history.truncate(10);
    }

    fn apply_query_template(&mut self, c: char) {
        let query = match c {
            '1' => "fields @timestamp, @message | sort @timestamp desc",
            '2' => "fields @timestamp, @message | filter @message like /ERROR/ | sort @timestamp desc",
            '3' => "fields @timestamp, @message | filter @message like /WARN/ | sort @timestamp desc",
            '4' => "fields @timestamp, @message | filter @message like /(?i)(exception|stacktrace|fatal)/ | sort @timestamp desc",
            '5' => "stats count(*) by @log_level\n| sort count desc",
            '6' => "fields @timestamp, @message\n| filter @message like /ERROR/\n| stats count(*) as err_count by @message\n| sort err_count desc\n| limit 20",
            '7' => {
                self.input_mode = InputMode::KeywordSearch;
                self.input.show("Search keyword", "type your keyword...");
                return;
            }
            '8' => "filter @duration > 0\n| stats avg(@duration) as avg_ms, max(@duration) as max_ms, p99(@duration) as p99_ms, count(*) as requests\n| sort avg_ms desc",
            '9' => "fields @timestamp, @message | sort @timestamp desc | limit 200",
            _ => return,
        };
        self.last_insights_query = query.replace('\n', " ");
        self.show_insights_query_input();
    }

    fn run_exec_interactive(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let (cmd, args) = if self.active_tab == TAB_SSM {
            let instance = match self.instances.selected() {
                Some(i) => i.id.clone(),
                None => {
                    self.set_error("No instance selected".to_string());
                    return Ok(());
                }
            };
            (
                self.aws_bin.clone(),
                vec![
                    "ssm".to_string(),
                    "start-session".to_string(),
                    "--target".to_string(),
                    instance,
                    "--profile".to_string(),
                    self.active_profile.clone().unwrap_or_default(),
                    "--region".to_string(),
                    self.active_region.clone(),
                ],
            )
        } else {
            let cluster = match &self.selected_cluster {
                Some(c) => c.clone(),
                None => {
                    self.set_error("No cluster selected".to_string());
                    return Ok(());
                }
            };
            let task = match self.tasks.selected() {
                Some(t) => t.clone(),
                None => {
                    self.set_error("No task selected".to_string());
                    return Ok(());
                }
            };
            let task_arn = task.task_arn.clone();
            let container = self
                .containers
                .selected()
                .map(|c| c.name.clone())
                .or_else(|| task.containers.first().map(|c| c.name.clone()));
            let container = match container {
                Some(c) => c,
                None => {
                    self.set_error("No container found in task".to_string());
                    return Ok(());
                }
            };
            (
                self.aws_bin.clone(),
                vec![
                    "ecs".to_string(),
                    "execute-command".to_string(),
                    "--cluster".to_string(),
                    cluster,
                    "--task".to_string(),
                    task_arn,
                    "--container".to_string(),
                    container,
                    "--interactive".to_string(),
                    "--command".to_string(),
                    self.pending_shell
                        .clone()
                        .unwrap_or_else(|| "/bin/sh".to_string()),
                    "--profile".to_string(),
                    self.active_profile.clone().unwrap_or_default(),
                    "--region".to_string(),
                    self.active_region.clone(),
                ],
            )
        };

        log::info!("exec: {} {}", cmd, args.join(" "));

        // Suspend TUI
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )?;

        let status = std::process::Command::new(&cmd)
            .args(&args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        match status {
            Ok(s) if !s.success() => {
                self.set_error(format!("Command exited with code {:?}", s.code()));
            }
            Err(e) => {
                self.set_error(format!("Command error: {e}"));
            }
            _ => {}
        }

        // Resume TUI
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        terminal.clear()?;

        Ok(())
    }

    fn run_install_session_plugin(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        // Suspend TUI
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )?;

        // Detect platform and install
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;

        let success = if os == "linux" {
            let deb_arch = if arch == "aarch64" { "arm64" } else { "64bit" };
            let url = format!(
                "https://s3.amazonaws.com/session-manager-downloads/plugin/latest/ubuntu_{deb_arch}/session-manager-plugin.deb"
            );
            println!("Downloading session-manager-plugin...");
            println!("URL: {url}");
            println!();

            let dl = std::process::Command::new("curl")
                .args(["-fSL", &url, "-o", "/tmp/session-manager-plugin.deb"])
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status();

            match dl {
                Ok(s) if s.success() => {
                    println!("\nInstalling (requires sudo)...");
                    let install = std::process::Command::new("sudo")
                        .args(["dpkg", "-i", "/tmp/session-manager-plugin.deb"])
                        .stdin(std::process::Stdio::inherit())
                        .stdout(std::process::Stdio::inherit())
                        .stderr(std::process::Stdio::inherit())
                        .status();

                    let _ = std::fs::remove_file("/tmp/session-manager-plugin.deb");

                    match install {
                        Ok(s) if s.success() => true,
                        Ok(s) => {
                            println!("\nInstallation failed (exit {:?})", s.code());
                            false
                        }
                        Err(e) => {
                            println!("\nInstallation error: {e}");
                            false
                        }
                    }
                }
                Ok(s) => {
                    println!("\nDownload failed (exit {:?})", s.code());
                    false
                }
                Err(e) => {
                    println!("\nDownload error: {e}");
                    false
                }
            }
        } else if os == "macos" {
            let url = "https://s3.amazonaws.com/session-manager-downloads/plugin/latest/mac/session-manager-plugin.pkg";
            println!("Downloading session-manager-plugin...");
            let dl = std::process::Command::new("curl")
                .args(["-fSL", url, "-o", "/tmp/session-manager-plugin.pkg"])
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status();

            match dl {
                Ok(s) if s.success() => {
                    println!("\nInstalling (requires sudo)...");
                    let install = std::process::Command::new("sudo")
                        .args([
                            "installer",
                            "-pkg",
                            "/tmp/session-manager-plugin.pkg",
                            "-target",
                            "/",
                        ])
                        .stdin(std::process::Stdio::inherit())
                        .stdout(std::process::Stdio::inherit())
                        .stderr(std::process::Stdio::inherit())
                        .status();

                    let _ = std::fs::remove_file("/tmp/session-manager-plugin.pkg");

                    matches!(install, Ok(s) if s.success())
                }
                _ => false,
            }
        } else {
            println!("Unsupported OS: {os}. Please install session-manager-plugin manually.");
            println!("See: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html");
            false
        };

        if success {
            println!("\nsession-manager-plugin installed successfully!");
            self.session_plugin_installed = true;
        } else {
            self.set_error("session-manager-plugin installation failed".to_string());
        }

        println!("\nPress Enter to return to lazy-aws...");
        let _ = std::io::stdin().read_line(&mut String::new());

        // Resume TUI
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        terminal.clear()?;

        Ok(())
    }

    // --- Background thread spawners ---

    fn spawn_load_clusters(&mut self) {
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        if self.loading_clusters {
            return;
        }
        self.loading_clusters = true;
        self.spinner.start("Loading clusters...");

        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_clusters() {
            Ok(clusters) => {
                let _ = tx.send(BgMsg::ClustersLoaded(clusters));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::ClustersError(e));
            }
        });
    }

    fn spawn_load_services(&mut self) {
        let cluster = match &self.selected_cluster {
            Some(c) => c.clone(),
            None => return,
        };
        if self.loading_services {
            return;
        }
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.loading_services = true;
        self.spinner.start("Loading services...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_services(&cluster) {
            Ok(services) => {
                let _ = tx.send(BgMsg::ServicesLoaded { services });
            }
            Err(e) => {
                let _ = tx.send(BgMsg::ServicesError(e));
            }
        });
    }

    fn spawn_load_tasks(&mut self) {
        let cluster = match &self.selected_cluster {
            Some(c) => c.clone(),
            None => return,
        };
        let service = match &self.selected_service {
            Some(s) => s.clone(),
            None => return,
        };
        if self.loading_tasks {
            return;
        }
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.loading_tasks = true;
        self.spinner.start("Loading tasks...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_tasks(&cluster, &service) {
            Ok(tasks) => {
                let _ = tx.send(BgMsg::TasksLoaded { tasks });
            }
            Err(e) => {
                let _ = tx.send(BgMsg::TasksError(e));
            }
        });
    }

    fn spawn_load_instances(&mut self) {
        if self.loading_instances {
            return;
        }
        self.loading_instances = true;
        self.spinner.start("Loading instances...");

        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_instances() {
            Ok(instances) => {
                let _ = tx.send(BgMsg::InstancesLoaded(instances));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::InstancesError(e));
            }
        });
    }

    fn spawn_load_log_groups(&mut self) {
        if self.loading_log_groups {
            return;
        }
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.loading_log_groups = true;
        self.spinner.start("Loading log groups...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_log_groups(Some("/ecs/")) {
            Ok(groups) => {
                let _ = tx.send(BgMsg::LogGroupsLoaded(groups));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::LogGroupsError(e));
            }
        });
    }

    fn spawn_load_log_streams(&mut self) {
        let group = match self.log_groups.selected() {
            Some(g) => g.log_group_name.clone(),
            None => return,
        };
        if self.loading_log_streams {
            return;
        }
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.loading_log_streams = true;
        self.spinner.start("Loading log streams...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_log_streams(&group) {
            Ok(streams) => {
                let _ = tx.send(BgMsg::LogStreamsLoaded { streams });
            }
            Err(e) => {
                let _ = tx.send(BgMsg::LogStreamsError(e));
            }
        });
    }

    fn spawn_load_rds_instances(&mut self) {
        if self.loading_rds_instances {
            return;
        }
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.loading_rds_instances = true;
        self.spinner.start("Loading RDS instances...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_db_instances() {
            Ok(instances) => {
                let _ = tx.send(BgMsg::DbInstancesLoaded(instances));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::DbInstancesError(e));
            }
        });
    }

    fn handle_rds_connect(&mut self) {
        let inst = match self.rds_instances.selected() {
            Some(i) => i,
            None => {
                self.set_error("No RDS instance selected".to_string());
                return;
            }
        };
        if inst.endpoint.is_none() {
            self.set_error("Instance has no endpoint (still creating?)".to_string());
            return;
        }
        self.choice_mode = ChoiceMode::RdsConnectMethod;
        self.choice.show(
            "Connect Method",
            vec![
                Choice {
                    key: '1',
                    label: "Direct connection".to_string(),
                },
                Choice {
                    key: '2',
                    label: "SSM tunnel".to_string(),
                },
            ],
        );
    }

    fn start_rds_connect_flow(&mut self) {
        let inst = match self.rds_instances.selected() {
            Some(i) => i,
            None => return,
        };
        let identifier = inst.db_instance_identifier.clone();
        let master_user = inst.master_username.clone();

        // Check for saved credentials (nested: profile -> rds -> identifier)
        let profile = self.active_profile.as_deref().unwrap_or("default");
        let saved_lookup = self
            .saved_credentials
            .profiles
            .get(profile)
            .and_then(|p| p.rds.get(&identifier));
        if let Some(saved) = saved_lookup {
            if let Ok(password) = credentials::decode_password(&saved.password) {
                self.pending_rds_user = saved.username.clone();
                self.pending_rds_password = password;
                self.credentials_just_saved = true; // don't re-prompt to save
                let database = saved.database.clone();
                self.finalize_rds_connection(database);
                return;
            }
        }

        // No saved credentials — normal flow
        self.credentials_just_saved = false;
        self.input_mode = InputMode::RdsUsername;
        if master_user.is_empty() {
            self.input.show("Username", "mysql username...");
        } else {
            self.input
                .show_with_value("Username", "mysql username...", &master_user);
        }
    }

    fn finalize_rds_connection(&mut self, database: Option<String>) {
        let inst = match self.rds_instances.selected() {
            Some(i) => i,
            None => return,
        };
        let endpoint = match &inst.endpoint {
            Some(e) => e,
            None => return,
        };

        // If SSM tunnel is active, connect via localhost
        let (host, port) = if let Some(local_port) = self.ssm_tunnel_local_port {
            ("127.0.0.1".to_string(), local_port)
        } else {
            (endpoint.address.clone(), endpoint.port)
        };

        let conn = RdsConnection {
            host,
            port,
            user: std::mem::take(&mut self.pending_rds_user),
            password: std::mem::take(&mut self.pending_rds_password),
            database,
        };

        self.rds_connection = Some(conn);
        self.spinner.start("Testing connection...");
        self.spawn_test_rds_connection();
    }

    fn start_ssm_tunnel_flow(&mut self) {
        // If SSM instances are already loaded (from SSM tab), show selector directly
        if !self.instances.instances.is_empty() {
            self.tunnel_ssm_instances = self.instances.instances.clone();
            self.show_ssm_instance_selector();
            return;
        }

        // Otherwise, load them in background first
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.spinner.start("Loading SSM instances...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_instances() {
            Ok(instances) => {
                let _ = tx.send(BgMsg::SsmInstancesForTunnel(instances));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::SsmInstancesForTunnelError(e));
            }
        });
    }

    fn show_ssm_instance_selector(&mut self) {
        if self.tunnel_ssm_instances.is_empty() {
            self.set_error("No SSM-managed instances found in this region".to_string());
            return;
        }
        let choices: Vec<Choice> = self
            .tunnel_ssm_instances
            .iter()
            .enumerate()
            .take(9)
            .map(|(i, inst)| {
                let key = (b'1' + i as u8) as char;
                let label = if inst.name.is_empty() {
                    inst.id.clone()
                } else {
                    format!("{} ({})", inst.name, inst.id)
                };
                Choice { key, label }
            })
            .collect();
        self.choice_mode = ChoiceMode::SsmInstanceSelector;
        self.choice.show("Select bastion instance", choices);
    }

    fn open_ssm_tunnel(&mut self, ssm_target: &str) {
        let inst = match self.rds_instances.selected() {
            Some(i) => i,
            None => return,
        };
        let endpoint = match &inst.endpoint {
            Some(e) => e,
            None => return,
        };

        // Find a free local port
        let local_port = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => {
                let port = listener
                    .local_addr()
                    .map(|a| a.port() as i32)
                    .unwrap_or(13306);
                drop(listener);
                port
            }
            Err(_) => 13306,
        };

        let remote_host = endpoint.address.clone();
        let remote_port = endpoint.port;

        log::info!(
            "starting SSM tunnel: {} -> {}:{} via {}",
            local_port,
            remote_host,
            remote_port,
            ssm_target
        );

        self.spinner.start("Opening SSM tunnel...");

        let remote_port_str = remote_port.to_string();
        let local_port_str = local_port.to_string();
        let params = format!(
            "{{\"host\":[\"{remote_host}\"],\"portNumber\":[\"{remote_port_str}\"],\"localPortNumber\":[\"{local_port_str}\"]}}"
        );

        let aws_bin = self.aws_bin.clone();
        let profile = self.active_profile.clone().unwrap_or_default();
        let region = self.active_region.clone();
        let ssm_target = ssm_target.to_string();

        let tx = self.bg_tx.clone();
        self.pending_ssm_tunnel = true;
        self.ssm_tunnel_local_port = Some(local_port);

        thread::spawn(move || {
            let child = std::process::Command::new(&aws_bin)
                .args([
                    "ssm",
                    "start-session",
                    "--target",
                    &ssm_target,
                    "--document-name",
                    "AWS-StartPortForwardingSessionToRemoteHost",
                    "--parameters",
                    &params,
                    "--profile",
                    &profile,
                    "--region",
                    &region,
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();

            match child {
                Ok(child) => {
                    let pid = child.id();
                    // Detach: forget the Child so its stdout/stderr pipes stay open
                    // and the process keeps running. We track it by PID and kill it
                    // via kill_process() on disconnect.
                    std::mem::forget(child);

                    // Wait for the tunnel to be ready, retry up to 3 times
                    for attempt in 0..3 {
                        std::thread::sleep(std::time::Duration::from_secs(2 + attempt));
                        let addr: std::net::SocketAddr =
                            format!("127.0.0.1:{local_port}").parse().unwrap();
                        if std::net::TcpStream::connect_timeout(
                            &addr,
                            std::time::Duration::from_secs(2),
                        )
                        .is_ok()
                        {
                            let _ = tx.send(BgMsg::SsmTunnelReady { pid });
                            return;
                        }
                    }
                    kill_process(pid);
                    let _ = tx.send(BgMsg::SsmTunnelError(format!(
                        "Tunnel started but port {local_port} not reachable after retries"
                    )));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::SsmTunnelError(format!(
                        "Failed to start tunnel: {e}"
                    )));
                }
            }
        });
    }

    fn kill_ssm_tunnel(&mut self) {
        if let Some(pid) = self.ssm_tunnel_pid.take() {
            log::info!("killing SSM tunnel pid {pid}");
            kill_process(pid);
        }
        self.ssm_tunnel_local_port = None;
        self.pending_ssm_tunnel = false;
    }

    fn spawn_test_rds_connection(&mut self) {
        let conn = match &self.rds_connection {
            Some(c) => c,
            None => return,
        };
        let host = conn.host.clone();
        let port = conn.port;
        let user = conn.user.clone();
        let password = conn.password.clone();
        let database = conn.database.clone();
        let tx = self.bg_tx.clone();

        thread::spawn(move || {
            log::info!("testing MySQL connection to {host}:{port} as {user}");
            let mut args = vec![
                "-h".to_string(),
                host,
                "-P".to_string(),
                port.to_string(),
                "-u".to_string(),
                user,
                format!("--password={password}"),
                "-e".to_string(),
                "SELECT 1".to_string(),
                "--batch".to_string(),
            ];
            if let Some(db) = database {
                args.push(db);
            }
            let output = std::process::Command::new("mysql").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    log::info!("MySQL connection OK");
                    let _ = tx.send(BgMsg::RdsConnectionOk);
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    log::error!("MySQL connection failed: {err}");
                    let _ = tx.send(BgMsg::RdsConnectionError(err));
                }
                Err(e) => {
                    log::error!("MySQL command error: {e}");
                    let _ = tx.send(BgMsg::RdsConnectionError(e.to_string()));
                }
            }
        });
    }

    fn spawn_load_rds_tables(&mut self) {
        let conn = match &self.rds_connection {
            Some(c) => c,
            None => return,
        };
        self.loading_rds_tables = true;
        self.spinner.start("Loading tables...");
        let host = conn.host.clone();
        let port = conn.port;
        let user = conn.user.clone();
        let password = conn.password.clone();
        let database = conn.database.clone();
        let tx = self.bg_tx.clone();

        thread::spawn(move || {
            let mut args = vec![
                "-h".to_string(),
                host,
                "-P".to_string(),
                port.to_string(),
                "-u".to_string(),
                user,
                format!("--password={password}"),
                "-e".to_string(),
                "SHOW TABLES".to_string(),
                "--batch".to_string(),
                "--raw".to_string(),
            ];
            if let Some(db) = database {
                args.push(db);
            }
            let output = std::process::Command::new("mysql").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    // Skip the header line (column name)
                    let tables: Vec<String> = text
                        .lines()
                        .skip(1)
                        .map(|l| l.to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    let _ = tx.send(BgMsg::RdsTablesLoaded(tables));
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    let _ = tx.send(BgMsg::RdsTablesError(err));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::RdsTablesError(e.to_string()));
                }
            }
        });
    }

    fn spawn_execute_query(&mut self, sql: &str) {
        let conn = match &self.rds_connection {
            Some(c) => c,
            None => return,
        };
        self.loading_query = true;
        self.spinner.start("Executing query...");
        let host = conn.host.clone();
        let port = conn.port;
        let user = conn.user.clone();
        let password = conn.password.clone();
        let database = conn.database.clone();
        let sql = sql.to_string();
        let tx = self.bg_tx.clone();

        thread::spawn(move || {
            let start = std::time::Instant::now();
            let mut args = vec![
                "-h".to_string(),
                host,
                "-P".to_string(),
                port.to_string(),
                "-u".to_string(),
                user,
                format!("--password={password}"),
                "-e".to_string(),
                sql.clone(),
                "--batch".to_string(),
                "--raw".to_string(),
            ];
            if let Some(db) = database {
                args.push(db);
            }
            let output = std::process::Command::new("mysql").args(&args).output();
            let elapsed = start.elapsed().as_millis() as u64;
            match output {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let (columns, rows) = parse_mysql_batch_output(&text);
                    let _ = tx.send(BgMsg::QueryResult {
                        columns,
                        rows,
                        duration_ms: elapsed,
                        query: sql,
                    });
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    let _ = tx.send(BgMsg::QueryError(err));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::QueryError(e.to_string()));
                }
            }
        });
    }

    fn spawn_preview_dml(&mut self, sql: &str) {
        let conn = match &self.rds_connection {
            Some(c) => c,
            None => return,
        };
        self.loading_query = true;
        self.spinner.start("Previewing query...");
        let host = conn.host.clone();
        let port = conn.port;
        let user = conn.user.clone();
        let password = conn.password.clone();
        let database = conn.database.clone();
        let sql = sql.to_string();
        let tx = self.bg_tx.clone();

        thread::spawn(move || {
            let preview_sql =
                format!("START TRANSACTION; {sql}; SELECT ROW_COUNT() AS 'affected'; ROLLBACK;");
            log::debug!("preview DML: mysql -e \"{preview_sql}\"");
            let mut args = vec![
                "-h".to_string(),
                host,
                "-P".to_string(),
                port.to_string(),
                "-u".to_string(),
                user,
                format!("--password={password}"),
                "-e".to_string(),
                preview_sql,
                "--batch".to_string(),
                "--raw".to_string(),
            ];
            if let Some(db) = database {
                args.push(db);
            }
            let output = std::process::Command::new("mysql").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    log::debug!("preview DML stdout: {text}");
                    // Parse: header line "affected" then value line "N"
                    let affected_rows = text
                        .lines()
                        .last()
                        .and_then(|line| line.trim().parse::<u64>().ok())
                        .unwrap_or(0);
                    let _ = tx.send(BgMsg::DmlPreview { sql, affected_rows });
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    log::error!("preview DML error: {err}");
                    let _ = tx.send(BgMsg::DmlPreviewError(err));
                }
                Err(e) => {
                    log::error!("preview DML failed: {e}");
                    let _ = tx.send(BgMsg::DmlPreviewError(e.to_string()));
                }
            }
        });
    }

    fn show_sql_query_input(&mut self) {
        self.input_mode = InputMode::SqlQuery;
        let history_hint = if self.sql_history.is_empty() {
            String::new()
        } else {
            format!("  [Shift+H: history ({})]", self.sql_history.len())
        };
        if self.last_sql_query.is_empty() {
            self.input
                .show(&format!("SQL Query{history_hint}"), "SELECT * FROM ...");
        } else {
            self.input.show_with_value(
                &format!("SQL Query{history_hint}"),
                "SELECT * FROM ...",
                &self.last_sql_query.clone(),
            );
        }
    }

    fn show_sql_modify_input(&mut self) {
        self.input_mode = InputMode::SqlModify;
        let history_hint = if self.sql_history.is_empty() {
            String::new()
        } else {
            format!("  [Shift+H: history ({})]", self.sql_history.len())
        };
        if self.last_sql_modify_query.is_empty() {
            self.input.show(
                &format!("SQL Execute{history_hint}"),
                "INSERT / UPDATE / DELETE / CREATE / DROP ...",
            );
        } else {
            self.input.show_with_value(
                &format!("SQL Execute{history_hint}"),
                "INSERT / UPDATE / DELETE / CREATE / DROP ...",
                &self.last_sql_modify_query.clone(),
            );
        }
    }

    fn show_sql_history(&mut self) {
        let choices: Vec<Choice> = self
            .sql_history
            .iter()
            .enumerate()
            .take(9)
            .map(|(i, q)| {
                let key = (b'1' + i as u8) as char;
                let label = if q.len() > 60 {
                    format!("{}...", &q[..60])
                } else {
                    q.clone()
                };
                Choice { key, label }
            })
            .collect();
        self.choice_mode = ChoiceMode::SqlHistory;
        self.choice.show("SQL History", choices);
    }

    fn add_to_sql_history(&mut self, query: &str) {
        self.sql_history.retain(|q| q != query);
        self.sql_history.insert(0, query.to_string());
        self.sql_history.truncate(10);
    }

    fn spawn_load_caller_identity(&mut self) {
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.get_caller_identity() {
            Ok(identity) => {
                let _ = tx.send(BgMsg::CallerIdentityLoaded(identity));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::CallerIdentityError(e));
            }
        });
    }

    fn spawn_load_aws_info(&mut self) {
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            if let Ok(version) = runner.version() {
                let _ = tx.send(BgMsg::AwsInfo { version });
            }
        });
    }

    fn spawn_load_profiles(&mut self) {
        let aws_bin = self.aws_bin.clone();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let output = std::process::Command::new(&aws_bin)
                .args(["configure", "list-profiles"])
                .output();
            match output {
                Ok(o) => {
                    let text = String::from_utf8_lossy(&o.stdout);
                    let profiles: Vec<String> = text
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    let _ = tx.send(BgMsg::ProfilesLoaded(profiles));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::ProfilesError(e.to_string()));
                }
            }
        });
    }

    fn show_profile_selector(&mut self) {
        if self.available_profiles.is_empty() {
            self.set_error("No profiles loaded yet".to_string());
            return;
        }

        let active = self.active_profile.as_deref().unwrap_or("");
        let choices: Vec<Choice> = self
            .available_profiles
            .iter()
            .enumerate()
            .take(9) // max '1' to '9'
            .map(|(i, name)| {
                let key = (b'1' + i as u8) as char;
                let marker = if name == active { " (active)" } else { "" };
                Choice {
                    key,
                    label: format!("{name}{marker}"),
                }
            })
            .collect();

        self.choice_mode = ChoiceMode::ProfileSelector;
        self.choice.show("Switch AWS Profile", choices);
    }

    // --- S3 methods ---

    fn spawn_load_buckets(&mut self) {
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        if self.loading_buckets {
            return;
        }
        self.loading_buckets = true;
        self.spinner.start("Loading buckets...");

        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_buckets() {
            Ok(buckets) => {
                let _ = tx.send(BgMsg::BucketsLoaded(buckets));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::BucketsError(e));
            }
        });
    }

    fn spawn_load_objects(&mut self) {
        let bucket = match &self.selected_bucket {
            Some(b) => b.clone(),
            None => return,
        };
        if self.loading_objects {
            return;
        }
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let prefix = self.objects.current_prefix.clone();
        self.loading_objects = true;
        self.spinner.start("Loading objects...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.list_objects(&bucket, &prefix) {
            Ok(result) => {
                let _ = tx.send(BgMsg::ObjectsLoaded {
                    result,
                    bucket,
                    prefix,
                });
            }
            Err(e) => {
                let _ = tx.send(BgMsg::ObjectsError(e));
            }
        });
    }

    fn handle_s3_download(&mut self) {
        let bucket = match &self.selected_bucket {
            Some(b) => b.clone(),
            None => {
                self.set_error("No bucket selected".to_string());
                return;
            }
        };
        let obj = match self.objects.selected() {
            Some(panels::S3ObjectItem::Object(o)) => o.clone(),
            _ => {
                self.set_error("Select an object to download".to_string());
                return;
            }
        };
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let filename = obj.key.rsplit('/').next().unwrap_or(&obj.key);
        let home = std::env::var("HOME").unwrap_or_default();
        let dest = format!("{home}/Downloads/{filename}");

        log::info!("downloading s3://{bucket}/{} to {dest}", obj.key);
        self.output.clear();
        self.output
            .append_line(&format!("Downloading {} → {dest}", obj.key));

        match runner.download_object(&bucket, &obj.key, &dest) {
            Ok(handle) => {
                self.stream_rx = Some(handle.rx);
                self.child_pid = handle.child_pid;
                self.spinner.start(&format!("Downloading {filename}..."));
            }
            Err(e) => {
                self.set_error(format!("Download failed: {e}"));
            }
        }
    }

    fn handle_s3_upload_prompt(&mut self) {
        if self.selected_bucket.is_none() {
            self.set_error("No bucket selected".to_string());
            return;
        }
        self.input_mode = InputMode::S3Upload;
        let cwd = std::env::current_dir()
            .map(|p| format!("{}/", p.display()))
            .unwrap_or_default();
        let bucket = self.selected_bucket.as_deref().unwrap_or("?");
        let prefix = &self.objects.current_prefix;
        self.input.show_with_value(
            &format!("Upload to s3://{bucket}/{prefix}"),
            "local file path...",
            &cwd,
        );
    }

    fn execute_s3_upload(&mut self, local_path: &str) {
        let bucket = match &self.selected_bucket {
            Some(b) => b.clone(),
            None => return,
        };
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        let filename = local_path.rsplit('/').next().unwrap_or(local_path);
        let key = format!("{}{}", self.objects.current_prefix, filename);

        log::info!("uploading {local_path} to s3://{bucket}/{key}");
        self.output.clear();
        self.output
            .append_line(&format!("Uploading {local_path} → s3://{bucket}/{key}"));

        match runner.upload_object(local_path, &bucket, &key) {
            Ok(handle) => {
                self.stream_rx = Some(handle.rx);
                self.child_pid = handle.child_pid;
                self.spinner.start(&format!("Uploading {filename}..."));
            }
            Err(e) => {
                self.set_error(format!("Upload failed: {e}"));
            }
        }
    }

    fn handle_s3_delete(&mut self) {
        let bucket = match &self.selected_bucket {
            Some(b) => b.clone(),
            None => {
                self.set_error("No bucket selected".to_string());
                return;
            }
        };
        match self.objects.selected() {
            Some(panels::S3ObjectItem::Object(obj)) => {
                let key = obj.key.clone();
                self.confirm.show(&format!("Delete s3://{bucket}/{key}?"));
                self.pending_action = Some(PendingAction::DeleteS3Object { bucket, key });
            }
            Some(panels::S3ObjectItem::Prefix(_)) => {
                self.set_error("Cannot delete prefixes".to_string());
            }
            _ => {}
        }
    }

    fn switch_profile(&mut self, profile: &str) {
        log::info!("switching to profile: {profile}");
        self.active_profile = Some(profile.to_string());

        // Resolve region from profile config, fallback to current region
        if let Some(region) = resolve_profile_region(profile) {
            log::info!("using region from profile config: {region}");
            self.active_region = region;
        }

        // Recreate runner with new profile and resolved region
        let exec = crate::aws::RealExecutor::new(&self.aws_bin, profile, &self.active_region);
        let runner = crate::aws::Runner::new(Box::new(exec));
        self.runner = Some(Arc::new(runner));

        // Clear all cached data
        self.clusters.set_clusters(vec![]);
        self.services.set_services(vec![]);
        self.tasks.set_tasks(vec![]);
        self.containers.set_containers(vec![]);
        self.instances.set_instances(vec![]);
        self.log_groups.set_groups(vec![]);
        self.log_streams.set_streams(vec![]);
        self.rds_instances.set_instances(vec![]);
        self.rds_tables.set_tables(vec![]);
        self.query_results.clear();
        self.rds_connection = None;
        self.kill_ssm_tunnel();
        self.caller_identity = None;
        self.selected_cluster = None;
        self.selected_service = None;
        self.selected_bucket = None;
        self.ssm_visited = false;
        self.logs_visited = false;
        self.rds_visited = false;
        self.s3_visited = false;
        self.buckets.set_buckets(vec![]);
        self.objects.clear();
        self.err = None;

        if is_sso_profile(profile) {
            // Test credentials first -- if valid, no need to re-login
            self.spawn_check_credentials_then_load();
        } else {
            self.spawn_load_caller_identity();
            self.spawn_load_clusters();
        }
    }

    /// Checks if credentials are valid for the current profile.
    /// If valid, loads data. If expired, triggers SSO login.
    fn spawn_check_credentials_then_load(&mut self) {
        let runner = match &self.runner {
            Some(r) => Arc::clone(r),
            None => return,
        };
        self.spinner.start("Checking credentials...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || match runner.get_caller_identity() {
            Ok(identity) => {
                let _ = tx.send(BgMsg::CallerIdentityLoaded(identity));
                let _ = tx.send(BgMsg::CredentialsValid);
            }
            Err(e) => {
                // "ForbiddenException" on GetRoleCredentials means the SSO token
                // is valid but the role lacks STS access (e.g. S3-only roles).
                // Only trigger re-login for actual token expiry errors.
                let is_token_expired = e.contains("ExpiredToken")
                    || e.contains("The SSO session")
                    || e.contains("Token has expired")
                    || e.contains("InvalidIdentityToken")
                    || e.contains("UnrecognizedClientException")
                    || e.contains("expired");

                if is_token_expired {
                    log::info!("credentials expired, triggering SSO login");
                    let _ = tx.send(BgMsg::CredentialsExpired);
                } else {
                    // Permission error (ForbiddenException, AccessDenied, etc.)
                    // Credentials are likely valid, proceed to load data.
                    log::info!(
                        "STS check failed with permission error, proceeding: {}",
                        e.lines().next().unwrap_or(&e)
                    );
                    let _ = tx.send(BgMsg::CredentialsValid);
                }
            }
        });
    }

    fn run_sso_login_interactive(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        // Suspend TUI
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )?;

        // Run `aws sso login --profile <profile>` interactively
        let profile = match &self.active_profile {
            Some(p) => p.clone(),
            None => {
                self.set_error("No profile selected".to_string());
                // Resume TUI before returning
                crossterm::terminal::enable_raw_mode()?;
                crossterm::execute!(
                    terminal.backend_mut(),
                    crossterm::terminal::EnterAlternateScreen,
                    crossterm::event::EnableMouseCapture
                )?;
                return Ok(());
            }
        };
        let args = vec![
            "sso".to_string(),
            "login".to_string(),
            "--profile".to_string(),
            profile.clone(),
        ];
        let cmd = self.aws_bin.clone();
        log::info!("running SSO login: {cmd} {}", args.join(" "));

        let status = std::process::Command::new(&cmd)
            .args(&args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        match status {
            Ok(s) if s.success() => {
                log::info!("SSO login successful");
            }
            Ok(s) => {
                log::warn!("SSO login exited with code: {:?}", s.code());
                self.set_error(format!("SSO login failed (exit {:?})", s.code()));
            }
            Err(e) => {
                log::error!("SSO login error: {e}");
                self.set_error(format!("SSO login error: {e}"));
            }
        }

        // Resume TUI
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        // Force full redraw — ratatui's internal buffer is stale after screen switch
        terminal.clear()?;

        // Reload data after login
        self.spawn_load_caller_identity();
        self.spawn_load_clusters();

        Ok(())
    }
}

/// Reads `~/.aws/config` to find the region for a given profile.
/// Looks for `region` or `sso_region` under `[profile <name>]` (or `[default]`).
fn resolve_profile_region(profile: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.aws/config");
    let content = std::fs::read_to_string(path).ok()?;

    let section_header = if profile == "default" {
        "[default]".to_string()
    } else {
        format!("[profile {profile}]")
    };

    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if key == "region" || key == "sso_region" {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn format_bucket_detail(bucket: &aws::Bucket) -> Vec<String> {
    vec![
        format!("Bucket        {}", bucket.name),
        format!("Created       {}", bucket.creation_date),
        String::new(),
        format!("ARN           arn:aws:s3:::{}", bucket.name),
    ]
}

fn format_object_detail(obj: &aws::S3Object, bucket: &str) -> Vec<String> {
    let size_str = crate::ui::panels::objects::format_size(obj.size);
    vec![
        format!("Key           {}", obj.key),
        format!("Bucket        {bucket}"),
        format!("Size          {} ({} bytes)", size_str, obj.size),
        format!("Modified      {}", obj.last_modified),
        format!("Storage       {}", obj.storage_class),
        format!("ETag          {}", obj.e_tag),
        String::new(),
        format!("URI           s3://{bucket}/{}", obj.key),
    ]
}

/// Checks if a profile uses SSO by looking for `sso_start_url` or `sso_session` in `~/.aws/config`.
fn format_cluster_detail(cluster: &aws::Cluster) -> Vec<String> {
    vec![
        format!("Name          {}", cluster.cluster_name),
        format!("ARN           {}", cluster.cluster_arn),
        format!("Status        {}", cluster.status),
        String::new(),
        "Tasks".to_string(),
        format!("  Running     {}", cluster.running_tasks_count),
        format!("  Pending     {}", cluster.pending_tasks_count),
        String::new(),
        format!("Services      {}", cluster.active_services_count),
        format!(
            "Instances     {}",
            cluster.registered_container_instances_count
        ),
    ]
}

fn format_service_detail(svc: &aws::Service) -> Vec<String> {
    let mut lines = vec![
        format!("Name          {}", svc.service_name),
        format!("ARN           {}", svc.service_arn),
        format!("Status        {}", svc.status),
        format!("Launch type   {}", svc.launch_type),
        format!("Task def      {}", svc.task_definition),
        String::new(),
        "Counts".to_string(),
        format!("  Desired     {}", svc.desired_count),
        format!("  Running     {}", svc.running_count),
        format!("  Pending     {}", svc.pending_count),
        String::new(),
        format!(
            "Exec enabled  {}",
            if svc.enable_execute_command {
                "yes"
            } else {
                "no"
            }
        ),
    ];

    if !svc.deployments.is_empty() {
        lines.push(String::new());
        lines.push("Deployments".to_string());
        for d in &svc.deployments {
            lines.push(format!(
                "  {} {} {}/{}",
                d.status, d.rollout_state, d.running_count, d.desired_count
            ));
            lines.push(format!("    Task def  {}", d.task_definition));
            if !d.created_at.is_empty() {
                lines.push(format!("    Created   {}", d.created_at));
            }
        }
    }

    if !svc.load_balancers.is_empty() {
        lines.push(String::new());
        lines.push("Load Balancers".to_string());
        for lb in &svc.load_balancers {
            lines.push(format!("  {}:{}", lb.container_name, lb.container_port));
            if !lb.target_group_arn.is_empty() {
                lines.push(format!("    TG  {}", lb.target_group_arn));
            }
        }
    }

    lines
}

fn format_task_detail(task: &aws::Task) -> Vec<String> {
    let short_id = task.task_arn.rsplit('/').next().unwrap_or(&task.task_arn);
    let mut lines = vec![
        format!("Task ID       {}", short_id),
        format!("ARN           {}", task.task_arn),
        format!("Status        {}", task.last_status),
        format!("Desired       {}", task.desired_status),
        format!("Launch type   {}", task.launch_type),
        format!("Task def      {}", task.task_definition_arn),
        format!(
            "Exec enabled  {}",
            if task.enable_execute_command {
                "yes"
            } else {
                "no"
            }
        ),
    ];

    if let Some(ref started) = task.started_at {
        lines.push(format!("Started       {}", started));
    }
    if let Some(ref conn) = task.connectivity {
        lines.push(format!("Connectivity  {}", conn));
    }
    if let Some(ref health) = task.health_status {
        lines.push(format!("Health        {}", health));
    }

    lines.push(String::new());
    lines.push(format!("Containers    {}", task.containers.len()));
    for c in &task.containers {
        let health = c.health_status.as_deref().unwrap_or("-");
        lines.push(format!(
            "  {} [{}] health:{}",
            c.name, c.last_status, health
        ));
        lines.push(format!("    Image  {}", c.image));
    }

    lines
}

fn format_container_detail(container: &aws::Container) -> Vec<String> {
    let mut lines = vec![
        format!("Name          {}", container.name),
        format!("Status        {}", container.last_status),
        format!("Image         {}", container.image),
    ];

    if let Some(ref health) = container.health_status {
        lines.push(format!("Health        {}", health));
    }
    if let Some(ref runtime_id) = container.runtime_id {
        lines.push(format!("Runtime ID    {}", runtime_id));
    }
    if let Some(ref arn) = container.container_arn {
        lines.push(String::new());
        lines.push(format!("ARN           {}", arn));
    }

    lines
}

fn format_instance_detail(inst: &aws::Instance) -> Vec<String> {
    let mut lines = vec![
        format!("Name          {}", inst.name),
        format!("Instance ID   {}", inst.id),
        format!("State         {}", inst.state),
        format!("Type          {}", inst.instance_type),
        format!("Platform      {}", inst.platform),
        format!("AZ            {}", inst.availability_zone),
        String::new(),
        format!("Private IP    {}", inst.private_ip),
    ];

    if let Some(ref ip) = inst.public_ip {
        lines.push(format!("Public IP     {}", ip));
    }

    lines.push(String::new());
    lines.push(format!("SSM Status    {}", inst.ssm_ping_status));

    if let Some(ref ver) = inst.ssm_agent_version {
        lines.push(format!("SSM Agent     {}", ver));
    }

    lines
}

fn is_modification_query(sql: &str) -> bool {
    let upper = sql.trim_start().to_uppercase();
    upper.starts_with("INSERT")
        || upper.starts_with("UPDATE")
        || upper.starts_with("DELETE")
        || upper.starts_with("DROP")
        || upper.starts_with("ALTER")
        || upper.starts_with("CREATE")
        || upper.starts_with("TRUNCATE")
        || upper.starts_with("RENAME")
}

fn is_dml_query(sql: &str) -> bool {
    let upper = sql.trim_start().to_uppercase();
    upper.starts_with("INSERT") || upper.starts_with("UPDATE") || upper.starts_with("DELETE")
}

fn parse_mysql_batch_output(text: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = text.lines();
    let columns: Vec<String> = match lines.next() {
        Some(header) => header.split('\t').map(|s| s.to_string()).collect(),
        None => return (vec![], vec![]),
    };
    let rows: Vec<Vec<String>> = lines
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(|s| s.to_string()).collect())
        .collect();
    (columns, rows)
}

fn format_rds_instance_detail(inst: &aws::DbInstance) -> Vec<String> {
    let mut lines = vec![
        format!("Identifier    {}", inst.db_instance_identifier),
        format!("ARN           {}", inst.db_instance_arn),
        format!("Engine        {} {}", inst.engine, inst.engine_version),
        format!("Status        {}", inst.db_instance_status),
        format!("Class         {}", inst.db_instance_class),
    ];

    if let Some(ref endpoint) = inst.endpoint {
        lines.push(String::new());
        lines.push(format!(
            "Endpoint      {}:{}",
            endpoint.address, endpoint.port
        ));
    }

    lines.push(String::new());
    lines.push(format!("Master user   {}", inst.master_username));
    if let Some(ref db_name) = inst.db_name {
        lines.push(format!("Database      {}", db_name));
    }

    lines.push(String::new());
    lines.push(format!(
        "Storage       {} GB ({})",
        inst.allocated_storage, inst.storage_type
    ));
    lines.push(format!(
        "Multi-AZ      {}",
        if inst.multi_az { "yes" } else { "no" }
    ));
    lines.push(format!(
        "Encrypted     {}",
        if inst.storage_encrypted { "yes" } else { "no" }
    ));
    lines.push(format!(
        "Public        {}",
        if inst.publicly_accessible {
            "yes"
        } else {
            "no"
        }
    ));
    lines.push(format!(
        "IAM Auth      {}",
        if inst.iam_database_authentication_enabled {
            "yes"
        } else {
            "no"
        }
    ));
    lines.push(format!("AZ            {}", inst.availability_zone));

    if let Some(ref subnet) = inst.db_subnet_group {
        lines.push(String::new());
        lines.push(format!("Subnet group  {}", subnet.db_subnet_group_name));
        lines.push(format!("VPC           {}", subnet.vpc_id));
    }

    if !inst.vpc_security_groups.is_empty() {
        lines.push(String::new());
        lines.push("Security Groups".to_string());
        for sg in &inst.vpc_security_groups {
            lines.push(format!("  {} ({})", sg.vpc_security_group_id, sg.status));
        }
    }

    if let Some(ref created) = inst.instance_create_time {
        lines.push(String::new());
        lines.push(format!("Created       {}", created));
    }

    lines
}

/// Parses a datetime string like "2024-04-13 10:30" into a Unix timestamp (UTC).
fn parse_datetime(s: &str) -> Option<i64> {
    // Try "YYYY-MM-DD HH:MM"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M") {
        return Some(dt.and_utc().timestamp());
    }
    // Try "YYYY-MM-DD HH:MM:SS"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    // Try "YYYY-MM-DD" (midnight)
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc().timestamp());
    }
    None
}

/// Copies text to the system clipboard. Tries wl-copy (Wayland), xclip, xsel in order.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    for (cmd, args) in candidates {
        if which::which(cmd).is_ok() {
            let mut child = Command::new(cmd)
                .args(*args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;

            if let Some(ref mut stdin) = child.stdin {
                use std::io::Write;
                stdin
                    .write_all(text.as_bytes())
                    .map_err(|e| e.to_string())?;
            }

            let status = child.wait().map_err(|e| e.to_string())?;
            if status.success() {
                return Ok(());
            }
        }
    }

    Err("No clipboard tool found (install wl-copy, xclip, or xsel)".to_string())
}

fn is_sso_profile(profile: &str) -> bool {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return false,
    };
    let path = format!("{home}/.aws/config");
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let section_header = if profile == "default" {
        "[default]".to_string()
    } else {
        format!("[profile {profile}]")
    };

    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some((key, _)) = trimmed.split_once('=') {
                let key = key.trim();
                if key == "sso_start_url" || key == "sso_session" {
                    return true;
                }
            }
        }
    }
    false
}

/// Creates a centered rectangle with percentage-based sizing.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
