mod config;
mod dispatched_info;
mod dispatcher;
mod platform_info;
mod publisher;
mod serve_dispatch;
mod server;
mod tethysdash_client;
mod trackdb_client;
use serve_dispatch::{dispatch, serve};

use crate::dispatched_info::DispatchedInfo;
use crate::server::health::get_health_status;

use clap::{Parser, Subcommand};

/// The odss2dash CLI
#[derive(Parser)]
#[command(
    version,
    name = "odss2dash",
    about = "odss2dash service",
    long_about = "odss2dash:
     The central function of this program is a service that relays
     platform positions from the TrackingDB/ODSS to TethysDash.",
    styles=cli_styles()
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Perform basic configuration checks
    #[command()]
    CheckConfig,

    /// Get all platforms from TrackingDB/ODSS
    #[command()]
    GetPlatforms,

    /// Get platform information from TrackingDB/ODSS
    #[command(arg_required_else_help = true)]
    GetPlatform {
        /// The platform ID
        platform_id: String,
    },

    /// Get platform positions from TrackingDB/ODSS
    #[command(arg_required_else_help = true)]
    GetPositions {
        /// The platform ID
        platform_id: String,
    },

    /// Add platforms to be dispatched
    #[command(arg_required_else_help = true)]
    AddDispatched {
        /// Platform IDs to dispatch
        platform_ids: Vec<String>,
    },

    /// Launch dispatch according to configuration
    #[command()]
    Dispatch {
        /// Run dispatch only once
        #[arg(long)]
        once: bool,
    },

    /// Launch service
    #[command()]
    Serve {
        /// Only run the server, not the dispatcher
        #[arg(long)]
        no_dispatch: bool,
    },

    /// Get health similar to the endpoint
    #[command()]
    Health,
}

fn main() {
    let args = Cli::parse();
    config::load_config();
    env_logger::init();
    match args.command {
        Commands::CheckConfig => {
            check_config();
        }
        Commands::GetPlatforms => {
            get_platforms();
        }
        Commands::GetPlatform { platform_id } => {
            get_platform(&platform_id);
        }
        Commands::GetPositions { platform_id } => {
            get_positions(&platform_id);
        }
        Commands::AddDispatched { platform_ids } => {
            add_dispatched(platform_ids);
        }
        Commands::Dispatch { once } => {
            dispatch(once);
        }
        Commands::Serve { no_dispatch } => {
            serve(no_dispatch);
        }
        Commands::Health => {
            get_health();
        }
    }
}

fn check_config() {
    println!("{}", config::get_config().redacted().json_string());
}

fn get_platforms() {
    log::info!("Getting platforms...");
    let platforms_res = trackdb_client::get_platforms();
    println!("{}", serde_json::to_string_pretty(&platforms_res).unwrap());
}

fn get_platform(platform_id: &str) {
    log::info!("Getting platform...");
    let platform_res = trackdb_client::get_platform(platform_id);
    if let Some(platform_res) = platform_res {
        println!("{}", serde_json::to_string_pretty(&platform_res).unwrap());
    } else {
        eprintln!("No platform by id: {platform_id}");
    }
}

fn get_positions(platform_id: &str) {
    let pos_res = trackdb_client::get_positions_per_config(platform_id);
    if let Some(pos_res) = pos_res {
        println!("{}", serde_json::to_string_pretty(&pos_res).unwrap());
    } else {
        eprintln!("No platform positions by id: {platform_id}");
    }
}

fn add_dispatched(platform_ids: Vec<String>) {
    DispatchedInfo::new().add_platform_ids(platform_ids);
}

fn get_health() {
    let status = get_health_status();
    println!("{}", serde_json::to_string_pretty(&status).unwrap());
}

fn cli_styles() -> clap::builder::Styles {
    use anstyle::{
        AnsiColor::{self, *},
        Color, Style,
    };
    fn style(color: AnsiColor) -> Style {
        Style::new().bold().fg_color(Some(Color::Ansi(color)))
    }
    clap::builder::Styles::styled()
        .usage(style(Yellow).underline())
        .header(style(Yellow).underline())
        .literal(style(Green))
        .placeholder(style(Blue))
}
