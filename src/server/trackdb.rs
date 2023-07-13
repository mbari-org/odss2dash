use crate::platform_info::PlatformInfo;
use crate::trackdb_client::{self, PlatformRes, PositionsResponse};

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing, Json, Router,
};
use hyper::StatusCode;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use utoipa::IntoParams;

pub fn create_trackdb_router(platform_info: Arc<Mutex<PlatformInfo>>) -> Router {
    Router::new()
        .route("/trackdb/platforms", routing::get(get_platforms))
        .route(
            "/trackdb/platforms/:platform_id",
            routing::get(get_platform),
        )
        .route(
            "/trackdb/platforms/:platform_id/positions",
            routing::get(get_platform_positions),
        )
        .with_state(platform_info)
}

/// Get all platforms from the Tracking DB.
#[utoipa::path(
    get,
    path = "/trackdb/platforms",
    responses(
       (status = 200, description = "List of platforms", body = PositionsResponse)
    )
)]
async fn get_platforms(
    State(platform_info): State<Arc<Mutex<PlatformInfo>>>,
) -> Json<Vec<PlatformRes>> {
    let mut platform_info = platform_info.lock().unwrap();
    let platforms_res = trackdb_client::get_platforms();
    if platforms_res.is_empty() {
        log::warn!("get_platforms: no platforms found; returning cached platforms");
    } else {
        log::info!("get_platforms: {} platforms found", platforms_res.len());
        platform_info.set_platforms(platforms_res);
    }
    Json(platform_info.get_platforms())
}

/// Get info about a platform.
#[utoipa::path(
    get,
    path = "/trackdb/platforms/{platform_id}",
    params(
        ("platform_id" = String, Path, description = "Platform ID"),
    ),
    responses(
       (status = 200, description = "Platform information", body = PlatformRes)
    )
)]
async fn get_platform(
    State(platform_info): State<Arc<Mutex<PlatformInfo>>>,
    Path(platform_id): Path<String>,
) -> impl IntoResponse {
    let mut platform_info = platform_info.lock().unwrap();
    match trackdb_client::get_platform(&platform_id) {
        Some(platform_res) => {
            log::debug!(
                "get_platform: updating cache for platform_id={}",
                platform_id
            );
            platform_info.update_platform(&platform_res);
            Json(platform_res).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Platform not found").into_response(),
    }
}

#[derive(Deserialize, IntoParams, Debug)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct PositionsQuery {
    /// Maximum number of last positions to report
    last_number_of_fixes: Option<u32>,
    /// Lower limit for time range restriction
    start_date: Option<String>,
    /// Upper limit for time range restriction
    end_date: Option<String>,
}

/// Get latest platform positions.
#[utoipa::path(
    get,
    path = "/trackdb/platforms/{platform_id}/positions",
    params(
        ("platform_id" = String, Path, description = "Platform ID"),
        PositionsQuery,
    ),
    responses(
       (status = 200, description = "List of positions", body = PositionsResponse)
    )
)]
async fn get_platform_positions(
    State(platform_info): State<Arc<Mutex<PlatformInfo>>>,
    Path(platform_id): Path<String>,
    query: Query<PositionsQuery>,
) -> impl IntoResponse {
    log::debug!("get_platform_positions: query={:?}", query);

    let query: PositionsQuery = query.0;
    log::debug!("query: {:?}", query);

    let platform_info = platform_info.lock().unwrap();

    let positions = trackdb_client::get_positions(
        &platform_id,
        query.last_number_of_fixes,
        query.start_date,
        query.end_date,
    );
    match positions {
        Some(pos_res) => {
            let platform_name = platform_info.get_platform(&platform_id).map(|p| p.name);
            Json(PositionsResponse {
                platform_name,
                ..pos_res
            })
            .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Platform not found").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{set_config, Config};
    use axum_test::*;
    use http::status::StatusCode;
    use pretty_assertions::assert_eq;

    fn odss_api() -> String {
        // TODO launch OdssApi mock server
        // ...
        // return mock server address:
        "https://odss.mbari.org/odss".to_string()
    }

    fn create_test_server() -> TestServer {
        set_config(Config {
            odss_api: odss_api(),
            default_last_number_of_fixes: 2,
            ..Config::default()
        });

        let platform_info = Arc::new(Mutex::new(PlatformInfo::default()));
        let app = create_trackdb_router(platform_info).into_make_service();

        TestServer::new(app).unwrap()
    }

    #[tokio::test]
    async fn it_should_get_platforms() {
        let server = create_test_server();

        let response = server.get("/trackdb/platforms").await;

        assert_eq!(response.status_code(), StatusCode::OK);
    }

    // #[tokio::test]
    // async fn it_should_return_404_for_bad_platform() {
    //     let server = create_test_server();
    //
    //     let response = server.get("/trackdb/platforms/BAD").await;
    //
    //     assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    // }

    // #[tokio::test]
    // async fn it_should_return_404_for_bad_platform_positions() {
    //     let server = create_test_server();
    //
    //     let response = server.get("/trackdb/platforms/BAD/positions").await;
    //
    //     println!("TEXT = {:?}", response.text());
    //     assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    // }
}
