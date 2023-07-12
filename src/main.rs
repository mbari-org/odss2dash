mod config;
mod trackdb_client;

use clap::{Parser, Subcommand};

/// The odss2dash CLI
#[derive(Parser)]
#[command(
    version,
    name = "odss2dash",
    about = "odss2dash service",
    styles = cli_styles()
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
}

fn main() {
    config::load_config();
    env_logger::init();
    let args = Cli::parse();
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
    }
}

fn check_config() {
    println!("{}", config::get_config().redacted().json_string());
}

fn get_platforms() {
    let platforms_res = trackdb_client::get_platforms();
    println!("{}", serde_json::to_string_pretty(&platforms_res).unwrap());
}

fn get_platform(platform_id: &str) {
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
