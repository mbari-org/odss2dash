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

#[derive(Deserialize, IntoParams, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlatformsQuery {
    /// Set to true to force reload from tracking db.
    refresh: Option<bool>,
}

/// Get all platforms from the Tracking DB.
#[utoipa::path(
    get,
    path = "/trackdb/platforms",
    params(
        PlatformsQuery,
    ),
    responses(
       (status = 200, description = "List of platforms", body = PositionsResponse)
    )
)]
async fn get_platforms(
    State(platform_info): State<Arc<Mutex<PlatformInfo>>>,
    query: Query<PlatformsQuery>,
) -> Json<Vec<PlatformRes>> {
    let query = query.0;
    log::info!("get_platforms query: {:?}", query);

    if query.refresh == Some(true) {
        let mut platform_info = platform_info.lock().unwrap();
        let platforms_res = trackdb_client::get_platforms();
        if platforms_res.is_empty() {
            log::warn!("get_platforms: no platforms found; returning cached platforms");
        } else {
            log::info!("get_platforms: {} platforms found", platforms_res.len());
            platform_info.set_platforms(platforms_res);
        }
        Json(platform_info.get_platforms())
    } else {
        let platforms = {
            let platform_info = platform_info.lock().unwrap();
            platform_info.get_platforms()
        };
        log::info!(
            "get_platforms: returned {} cached platforms",
            platforms.len()
        );
        Json(platforms)
    }
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
    log::info!("get_platform: platform_id={}", platform_id);
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
///
/// Note that the `lastNumberOfFixes` parameter has precedence over, and in fact is
/// mutually exclusive with the `startDate` and `endDate` parameters in the ODSS system.
/// The odss2dash service gives precedence to `startDate` and `endDate` to facilitate
/// the playback feature in the DashUI. That is, `lastNumberOfFixes` will not be passed
/// to the request to ODSS if any of `startDate` or `endDate` is given,
/// in which case, odss2dash will apply the `lastNumberOfFixes` limit on the response
/// from ODSS prior to responding to this request.
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
    let query = query.0;
    log::info!(
        "get_platform_positions: platform_id={platform_id} query: {:?}",
        query
    );

    let platform_info = get_platform_info(platform_info);

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

fn get_platform_info(platform_info: Arc<Mutex<PlatformInfo>>) -> PlatformInfo {
    platform_info.lock().unwrap().clone()
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
