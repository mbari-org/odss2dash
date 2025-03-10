mod dispatched;
pub mod health;
mod rapidoc;
mod swagger;
mod trackdb;

use crate::config;
use crate::dispatched_info::DispatchedInfo;
use crate::platform_info::PlatformInfo;
use crate::trackdb_client;

use axum::Router;
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{mpsc, Arc, Mutex};
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;

pub fn launch_server(
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
    done_sender: Option<mpsc::Sender<()>>,
) {
    match launch(platform_info, dispatched_info, done_sender) {
        Ok(()) => (),
        Err(e) => eprintln!("error launching server: {e}"),
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "odss2dash API"),
    paths(
        health::get_health,
        dispatched::get_dispatched_platforms,
        dispatched::get_dispatched_platform,
        dispatched::add_dispatched_platforms,
        dispatched::delete_dispatched_platform,
        trackdb::get_platforms,
        trackdb::get_platform,
        trackdb::get_platform_positions,
    ),
    components(
        schemas(
            health::HealthStatus,
            dispatched::PlatformAdd,
            dispatched::PlatformDeleteRes,
            trackdb_client::PlatformRes,
            trackdb_client::PositionsResponse,
            trackdb_client::Position,
        ),
    ),
    tags(
        (name = "health", description = "Basic service status"),
        (name = "dispatched", description = "Dispatched platforms for position notifications"),
        (name = "trackdb", description = "Tracking DB platform information"),
    )
)]
struct ApiDoc;

#[tokio::main]
async fn launch(
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
    done_sender: Option<mpsc::Sender<()>>,
) -> Result<(), Box<dyn Error>> {
    // To report the router paths that are set up:
    let mut paths: Vec<(&str, &str)> = vec![];

    let api_router = {
        let health_router = health::create_health_router();
        let dispatched_router =
            dispatched::create_dispatched_router(Arc::clone(&platform_info), dispatched_info);
        let trackdb_router = trackdb::create_trackdb_router(Arc::clone(&platform_info));
        let api_path = "/api";
        paths.push(("API", api_path));
        Router::new().nest(
            api_path,
            Router::new()
                .merge(health_router)
                .merge(dispatched_router)
                .merge(trackdb_router),
        )
    };

    // The complete app router:
    let app = {
        let cors = CorsLayer::permissive(); // TODO not so permissive
        api_router.merge(get_openapi_router(&mut paths)).layer(cors)
    };

    // Start the server:
    let config = config::get_config();
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let listener = TcpListener::bind(address.to_string()).await?;
    println!("Server listening on {}", address);
    for (name, path) in paths {
        println!("  {name:8} : {}{path}", &config.external_url);
    }

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    if let Some(done_sender) = done_sender {
        done_sender.send(()).expect("error sending done message")
    }

    Ok(())
}

fn get_openapi_router(paths: &mut Vec<(&str, &str)>) -> Router {
    let openapi_json_path = "/openapi.json";

    let openapi_router: Router = {
        paths.push(("OpenApi", openapi_json_path));
        let external_url = config::get_config().external_url.clone();
        let mut openapi = ApiDoc::openapi();
        openapi.servers = {
            let api_url = format!("{}/api", external_url);
            Some(vec![utoipa::openapi::Server::new(&api_url)])
        };
        Router::new().route(
            openapi_json_path,
            axum::routing::get(
                || async move { Ok::<_, std::convert::Infallible>(axum::Json(openapi)) },
            ),
        )
    };

    // json_rel for appropriate dispatch of doc UIs on deployed site:
    let json_rel = {
        let config = config::get_config();
        if config.external_url.ends_with("/odss2dash") {
            // For deployed site, need to prefix with /odss2dash/
            // per proxy setting on target server:
            format!("/odss2dash{openapi_json_path}")
        } else {
            openapi_json_path.to_string()
        }
    };

    let swagger_router = {
        let swagger_path = "/apidoc/";
        paths.push(("Swagger", swagger_path));
        swagger::create_swagger_router(&json_rel, swagger_path)
    };

    let rapidoc_router = {
        let rapidoc_path = "/rapidoc/";
        paths.push(("Rapidoc", rapidoc_path));
        rapidoc::create_rapidoc_router(&json_rel, rapidoc_path)
    };

    Router::new()
        .merge(openapi_router)
        .merge(swagger_router)
        .merge(rapidoc_router)
}

// https://github.com/tokio-rs/axum/blob/6c9cabf985236e3775fc07b3f54d639553fd1424/examples/graceful-shutdown/src/main.rs#L50
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Ctrl+C handler installed");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("signal handler installed")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    let bye = || println!("\nBye!");
    tokio::select! {
        _ = ctrl_c => bye(),
        _ = terminate => bye(),
    }
}
