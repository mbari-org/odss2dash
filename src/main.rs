mod config;
mod dispatched_info;
mod dispatcher;
mod platform_info;
mod publisher;
mod server;
mod tethysdash_client;
mod trackdb_client;
use clap::{Parser, Subcommand};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use crate::dispatched_info::DispatchedInfo;
use crate::dispatcher::Dispatcher;
use crate::platform_info::PlatformInfo;
use crate::tethysdash_client::post_xevent;

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
    }
}

fn check_config() {
    println!("{}", config::get_config().redacted().json_string());
}

fn get_platforms() {
    println!("Getting platforms...");
    let platforms_res = trackdb_client::get_platforms();
    println!("{}", serde_json::to_string_pretty(&platforms_res).unwrap());
}

fn get_platform(platform_id: &str) {
    println!("Getting platform...");
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

/// Initializes platform info cache via query to TrackingDB/ODSS.
fn create_platform_info() -> Arc<Mutex<PlatformInfo>> {
    fn init_platform_info(platform_info: &Arc<Mutex<PlatformInfo>>) {
        let platforms_res = trackdb_client::get_platforms();
        if platforms_res.is_empty() {
            eprintln!("warning: no platforms returned from TrackingDB/ODSS");
        } else {
            println!(
                "Platform cache initialized with {} platforms found in TrackingDB/ODSS",
                platforms_res.len()
            );
            platform_info.lock().unwrap().set_platforms(platforms_res);
        }
    }

    let platform_info = Arc::new(Mutex::new(PlatformInfo::default()));
    init_platform_info(&platform_info);
    platform_info
}

fn create_dispatched_info() -> Arc<Mutex<DispatchedInfo>> {
    Arc::new(Mutex::new(DispatchedInfo::new()))
}

fn create_dispatcher(
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
) -> Dispatcher {
    Dispatcher::new(post_xevent, platform_info, dispatched_info)
}

fn dispatch(once: bool) {
    let platform_info = create_platform_info();
    let dispatched_info = create_dispatched_info();
    let dispatcher = create_dispatcher(platform_info, dispatched_info);
    if once {
        dispatcher.launch_one_dispatch();
    } else {
        dispatcher.launch_dispatch(None);
    }
}

fn serve(no_dispatch: bool) {
    if no_dispatch {
        serve_only();
    } else {
        serve_and_dispatch();
    }
}

fn serve_only() {
    let platform_info = create_platform_info();
    let dispatched_info = create_dispatched_info();
    server::launch_server(platform_info, dispatched_info, None);
}

fn serve_and_dispatch() {
    let platform_info = create_platform_info();
    let dispatched_info = create_dispatched_info();

    let (done_sender, done_receiver) = mpsc::channel();

    let server_handle = {
        let platform_info = Arc::clone(&platform_info);
        let dispatched_info = Arc::clone(&dispatched_info);
        thread::spawn(move || {
            server::launch_server(platform_info, dispatched_info, Some(done_sender));
        })
    };

    let dispatch_handle = {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            create_dispatcher(platform_info, dispatched_info).launch_dispatch(Some(done_receiver));
        })
    };

    server_handle.join().unwrap();
    dispatch_handle.join().unwrap();
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
