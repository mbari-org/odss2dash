mod config;
mod dispatched_info;
mod dispatcher;
mod http;
mod platform_info;
mod publisher;
mod rt;
mod serve_dispatch;
mod server;
mod styles;
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
    styles=styles::clap_styles()
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

    /// Get health similar to the endpoint, as this process sees it
    #[command()]
    Health,

    /// Check the running service, as the container healthcheck does
    #[command()]
    Healthcheck,
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
        Commands::Healthcheck => {
            healthcheck();
        }
    }
}

fn check_config() {
    let config = config::get_config();
    println!("{}", config.redacted().json_string());

    println!("\nTethysDash API key expirations:");
    let now = chrono::Utc::now();
    let styles = styles::Styles::for_stdout();
    for td in &config.tethysdashes {
        println!(
            "  - {:10} : {}",
            td.name,
            publisher::api_key_expiration(&td.api_key, now, &styles)
        );
    }
}

fn get_platforms() {
    log::info!("Getting platforms...");
    match rt::block_on(trackdb_client::get_platforms()) {
        Ok(platforms_res) => println!("{}", serde_json::to_string_pretty(&platforms_res).unwrap()),
        Err(e) => eprintln!("error getting platforms: {e}"),
    }
}

fn get_platform(platform_id: &str) {
    log::info!("Getting platform...");
    match rt::block_on(trackdb_client::get_platform(platform_id)) {
        Ok(Some(platform_res)) => {
            println!("{}", serde_json::to_string_pretty(&platform_res).unwrap())
        }
        Ok(None) => eprintln!("No platform by id: {platform_id}"),
        Err(e) => eprintln!("error getting platform {platform_id}: {e}"),
    }
}

fn get_positions(platform_id: &str) {
    match rt::block_on(trackdb_client::get_positions_per_config(platform_id)) {
        Ok(pos_res) => println!("{}", serde_json::to_string_pretty(&pos_res).unwrap()),
        Err(e) => eprintln!("error getting positions for platform {platform_id}: {e}"),
    }
}

fn add_dispatched(platform_ids: Vec<String>) {
    DispatchedInfo::new().add_platform_ids(platform_ids);
}

fn get_health() {
    let status = get_health_status();
    println!("{}", serde_json::to_string_pretty(&status).unwrap());
}

/// Requests the health endpoint of the service running on this host.
/// Unlike [`get_health`], this says whether the service is actually serving.
fn healthcheck() {
    // The server binds 0.0.0.0, so this is the address of its own port, and not of a
    // name that could resolve to an address it is not listening on:
    let url = format!("http://127.0.0.1:{}/api/health", config::get_config().port);
    match rt::block_on(http::get_health(&url)) {
        Ok(body) => println!("{url}: {body}"),
        Err(e) => {
            eprintln!("{url}: {e}");
            std::process::exit(1);
        }
    }
}
