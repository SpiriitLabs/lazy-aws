use clap::Parser;
use std::process;

/// A TUI for interacting with AWS services, inspired by lazygit.
#[derive(Parser)]
#[command(about)]
#[command(version = long_version())]
struct Cli {
    /// AWS profile to use (overrides AWS_PROFILE)
    #[arg(short, long)]
    profile: Option<String>,

    /// AWS region to use (overrides AWS_REGION)
    #[arg(short, long)]
    region: Option<String>,

    /// Use light theme (for light terminal backgrounds)
    #[arg(long)]
    light: bool,
}

fn long_version() -> &'static str {
    concat!(
        "version=",
        env!("CARGO_PKG_VERSION"),
        ", commit=",
        env!("LA_GIT_COMMIT"),
        ", build date=",
        env!("LA_BUILD_DATE"),
        ", os=",
        env!("LA_OS"),
        ", arch=",
        env!("LA_ARCH"),
    )
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = lazy_aws::logger::init() {
        eprintln!("Warning: could not init logger: {e}");
    }

    let cfg = match lazy_aws::config::resolve(cli.profile.as_deref(), cli.region.as_deref()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    if cli.light {
        lazy_aws::ui::style::theme::set_mode(lazy_aws::ui::style::theme::ThemeMode::Light);
    } else {
        // Auto-detect terminal background
        let detected = lazy_aws::ui::style::theme::detect_mode();
        lazy_aws::ui::style::theme::set_mode(detected);
        log::info!("detected theme: {:?}", detected);
    }

    log::info!(
        "starting lazy-aws with profile={:?}, region={}",
        cfg.profile,
        cfg.region
    );

    let mut app = lazy_aws::ui::app::App::new(cfg.aws_bin, cfg.profile, cfg.region);

    if let Err(e) = app.run() {
        log::error!("program error: {e}");
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
