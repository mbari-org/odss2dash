mod dispatched;
mod trackdb;

use crate::config;
use crate::dispatched_info::DispatchedInfo;
use crate::platform_info::PlatformInfo;
use crate::trackdb_client;

use axum::{Router, Server};
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{mpsc, Arc, Mutex};
use tokio_shutdown::Shutdown;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

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
            dispatched::PlatformAdd,
            dispatched::PlatformDeleteRes,
            trackdb_client::PlatformRes,
            trackdb_client::PositionsResponse,
            trackdb_client::Position,
        ),
    ),
    tags(
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
    let config = config::get_config();
    let mut doc = ApiDoc::openapi();
    doc.servers = Some(vec![
        utoipa::openapi::Server::new(&config.external_url),
        utoipa::openapi::Server::new(format!("http://localhost:{}", config.port)),
    ]);

    let dispatched_router =
        dispatched::create_dispatched_router(Arc::clone(&platform_info), dispatched_info);

    let trackdb_router = trackdb::create_trackdb_router(Arc::clone(&platform_info));

    let apidoc_path = "/apidoc";
    let swagger_ui_router = SwaggerUi::new(apidoc_path)
        .url("/api-docs/openapi.json", doc)
        .config(utoipa_swagger_ui::Config::default().use_base_layout());

    let app = Router::new()
        .merge(dispatched_router)
        .merge(trackdb_router)
        .merge(swagger_ui_router);

    let shutdown = Shutdown::new().unwrap();

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    println!(
        "Listening on {}  (apidoc: http://localhost:{}{} -> external: {})",
        address, config.port, apidoc_path, config.external_url
    );
    let server = Server::try_bind(&address)?.serve(app.into_make_service());

    server.with_graceful_shutdown(shutdown.handle()).await?;

    if let Some(done_sender) = done_sender {
        done_sender.send(()).expect("error sending done message")
    }

    Ok(())
}
