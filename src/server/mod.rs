mod dispatched;
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
use tower_http::cors::CorsLayer;
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
    let dispatched_router =
        dispatched::create_dispatched_router(Arc::clone(&platform_info), dispatched_info);

    let trackdb_router = trackdb::create_trackdb_router(Arc::clone(&platform_info));

    let cors = CorsLayer::permissive(); // TODO not so permissive

    let app = Router::new()
        .nest(
            "/api",
            Router::new().merge(dispatched_router).merge(trackdb_router),
        )
        .merge(create_swagger_router())
        .layer(cors); // "first add your routes [...] and then call layer"

    let config = config::get_config();

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let listener = TcpListener::bind(address.to_string()).await?;
    println!("Server listening on {}", address);

    let server = axum::serve(listener, app.into_make_service());
    server.await?;
    if let Some(done_sender) = done_sender {
        done_sender.send(()).expect("error sending done message")
    }

    Ok(())
}

fn create_swagger_router() -> SwaggerUi {
    let config = config::get_config();

    let api_url = format!("{}/api", config.external_url);

    let mut doc = ApiDoc::openapi();
    doc.servers = Some(vec![utoipa::openapi::Server::new(&api_url)]);

    // For appropriate dispatch of SwaggerUI on deployed site:

    // (a) this value good for both local and deployed site:
    let apidoc_rel = "/apidoc";

    let json_rel = if config.external_url.ends_with("/odss2dash") {
        // (b) for deployed site, need to prefix with /odss2dash/
        // per proxy setting on target server:
        "/odss2dash/api-docs/openapi.json"
    } else {
        "/api-docs/openapi.json"
    };

    // (c) use the value in (b) for Config::from(), so that the correct url
    // is used by swagger-ui app (setting in swagger-initializer.js):
    let swagger_ui_config = utoipa_swagger_ui::Config::from(json_rel)
        .display_operation_id(true)
        .use_base_layout();

    let swagger_ui = SwaggerUi::new(apidoc_rel)
        // (d) as with (a), value here is good in general:
        .url("/api-docs/openapi.json", doc)
        .config(swagger_ui_config);

    println!("api : {}", &api_url);
    println!("doc : {}/apidoc/", config.external_url);
    println!("spec: {}/api-docs/openapi.json", config.external_url);

    swagger_ui
}
