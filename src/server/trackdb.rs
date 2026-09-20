use crate::platform_info::PlatformInfo;
use crate::trackdb_client::{self, FetchError, PlatformRes, PositionsRequest, PositionsResponse};

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
            "/trackdb/platforms/{platform_id}",
            routing::get(get_platform),
        )
        .route(
            "/trackdb/platforms/{platform_id}/positions",
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
        // Note: the request to ODSS is made before taking the lock, so that a slow
        // response does not hold up every other request needing the platform cache.
        let platforms_res = trackdb_client::get_platforms().await;
        let platforms = {
            let mut platform_info = platform_info.lock().unwrap();
            match platforms_res {
                Ok(platforms_res) if platforms_res.is_empty() => {
                    log::warn!("get_platforms: no platforms found; returning cached platforms");
                }
                Ok(platforms_res) => {
                    log::info!("get_platforms: {} platforms found", platforms_res.len());
                    platform_info.set_platforms(platforms_res);
                }
                Err(e) => {
                    log::warn!("get_platforms: {e}; returning cached platforms");
                }
            }
            platform_info.get_platforms()
        };
        Json(platforms)
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
       (status = 200, description = "Platform information", body = PlatformRes),
       (status = 404, description = "Platform not found"),
       (status = 502, description = "The Tracking DB could not be reached, or answered unusably"),
       (status = 504, description = "The Tracking DB did not respond in time"),
    )
)]
async fn get_platform(
    State(platform_info): State<Arc<Mutex<PlatformInfo>>>,
    Path(platform_id): Path<String>,
) -> impl IntoResponse {
    log::info!("get_platform: platform_id={}", platform_id);
    match trackdb_client::get_platform(&platform_id).await {
        Ok(Some(platform_res)) => {
            log::debug!(
                "get_platform: updating cache for platform_id={}",
                platform_id
            );
            platform_info.lock().unwrap().update_platform(&platform_res);
            Json(platform_res).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Platform not found").into_response(),
        Err(e) => fetch_error_response(e),
    }
}

/// Reports a failed request to ODSS as such, so that it is not confused with
/// the platform not existing.
fn fetch_error_response(e: FetchError) -> axum::response::Response {
    let status = match e {
        FetchError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        FetchError::Upstream(_) => StatusCode::BAD_GATEWAY,
    };
    (status, format!("Tracking DB request failed: {e}")).into_response()
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
/// This service accommodates the logic that ODSS currently implements for such queries,
/// where `lastNumberOfFixes` is mutually exclusive with, and takes precedence over,
/// `startDate` and `endDate`. Here the dates take precedence instead, so that a period can
/// be requested at all, and the parameters express two different requests:
///
/// - **A period**, by giving `startDate`, and optionally `endDate` (defaulting to now).
///   About `lastNumberOfFixes` positions covering the whole period are reported. If the
///   period holds no more than that, they are all of them; otherwise the most recent third
///   is reported position by position and the rest of the period is strided, that is, one
///   position every so many. The stride is over the positions, not over time, so the older
///   part comes out evenly spaced along the track rather than in time, and the response may
///   hold somewhat more positions than requested.
/// - **The latest positions**, by omitting `startDate`. The most recent
///   `lastNumberOfFixes` positions are reported, those at or before `endDate` if it is
///   given. Only a configured window back from that moment is considered (7 days by
///   default), so a platform that has not reported within it is reported as having no
///   positions rather than with older ones.
#[utoipa::path(
    get,
    path = "/trackdb/platforms/{platform_id}/positions",
    params(
        ("platform_id" = String, Path, description = "Platform ID"),
        PositionsQuery,
    ),
    responses(
       (status = 200, description = "List of positions", body = PositionsResponse),
       (status = 404, description = "Platform not found"),
       (status = 502, description = "The Tracking DB could not be reached, or answered unusably"),
       (status = 504, description = "The Tracking DB did not respond in time"),
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

    // A startDate given by the caller delimits a period; without one the request is for
    // the latest positions as of endDate, or as of now.
    let request = match query.start_date {
        Some(start_date) => PositionsRequest::Period {
            limit: query.last_number_of_fixes,
            start_date,
            end_date: query.end_date,
        },
        None => PositionsRequest::Latest {
            limit: query.last_number_of_fixes,
            end_date: query.end_date,
        },
    };

    let positions = trackdb_client::get_positions(&platform_id, request).await;
    match positions {
        Ok(pos_res) => {
            let platform_name = platform_info.get_platform(&platform_id).map(|p| p.name);
            Json(PositionsResponse {
                platform_name,
                ..pos_res
            })
            .into_response()
        }
        // A timeout says nothing about the platform existing:
        Err(FetchError::Timeout) => fetch_error_response(FetchError::Timeout),
        // Otherwise, ODSS reports an unknown platform as a failed request, so the cache
        // is what distinguishes "no such platform" from "ODSS could not answer now":
        Err(e) if platform_info.get_platform(&platform_id).is_none() => {
            log::info!("get_platform_positions: unknown platform_id={platform_id} ({e})");
            (StatusCode::NOT_FOUND, "Platform not found").into_response()
        }
        Err(e) => fetch_error_response(e),
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
            ..Config::default()
        });

        let platform_info = Arc::new(Mutex::new(PlatformInfo::default()));
        let app = create_trackdb_router(platform_info).into_make_service();

        TestServer::new(app)
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
