use crate::dispatched_info::DispatchedInfo;
use crate::platform_info::PlatformInfo;
use crate::trackdb_client::PlatformRes;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing, Json, Router,
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct Info {
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
}

pub fn create_dispatched_router(
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
) -> Router {
    let info = Arc::new(Mutex::new(Info {
        platform_info,
        dispatched_info,
    }));

    Router::new()
        .route(
            "/dispatched/platforms",
            routing::get(get_dispatched_platforms),
        )
        .route(
            "/dispatched/platforms/:platform_id",
            routing::get(get_dispatched_platform),
        )
        .route(
            "/dispatched/platforms",
            routing::post(add_dispatched_platforms),
        )
        .route(
            "/dispatched/platforms/:platform_id",
            routing::delete(delete_dispatched_platform),
        )
        .with_state(info)
}

/// Get all dispatched platforms.
#[utoipa::path(
    get,
    path = "/dispatched/platforms",
    responses(
       (status = 200, description = "List of dispatched platforms", body = Vec<PlatformRes>)
    )
)]
async fn get_dispatched_platforms(State(info): State<Arc<Mutex<Info>>>) -> Json<Vec<PlatformRes>> {
    let info = info.lock().unwrap();
    let mut dispatched_info = info.dispatched_info.lock().unwrap();
    let platform_ids = dispatched_info.get_platform_ids();
    let platform_info = info.platform_info.lock().unwrap();
    let mut platforms_res: Vec<PlatformRes> = Vec::new();
    for platform_id in platform_ids {
        match platform_info.get_platform(&platform_id) {
            Some(platform_res) => platforms_res.push(platform_res),
            None => {
                log::debug!("Platform not found, so no longer dispatched: {platform_id}");
                dispatched_info.delete_platform_id(&platform_id);
            }
        }
    }
    Json(platforms_res)
}

/// Get info about a dispatched platform.
#[utoipa::path(
    get,
    path = "/dispatched/platforms/{platform_id}",
    params(
        ("platform_id" = String, Path, description = "Platform ID"),
    ),
    responses(
       (status = 200, description = "Information of dispatched platform", body = PlatformRes)
    )
)]
async fn get_dispatched_platform(
    State(info): State<Arc<Mutex<Info>>>,
    Path(platform_id): Path<String>,
) -> impl IntoResponse {
    log::info!("get_dispatched_platform: platform_id={}", platform_id);
    let info = info.lock().unwrap();
    let mut dispatched_info = info.dispatched_info.lock().unwrap();
    match dispatched_info.is_dispatched_platform(&platform_id) {
        true => {
            let platform_info = info.platform_info.lock().unwrap();
            match platform_info.get_platform(&platform_id) {
                Some(platform_res) => Json(platform_res).into_response(),
                None => {
                    log::debug!("Platform not found, so no longer dispatched: {platform_id}");
                    dispatched_info.delete_platform_id(&platform_id);
                    (
                        StatusCode::NOT_FOUND,
                        "Platform not found, so no longer dispatched",
                    )
                        .into_response()
                }
            }
        }
        false => (StatusCode::NOT_FOUND, "Platform not found").into_response(),
    }
}

/// Platform IDs to add for dispatching.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAdd {
    platform_ids: Vec<String>,
}

/// Add platforms to be dispatched.
#[utoipa::path(
    post,
    path = "/dispatched/platforms",
    request_body = PlatformAdd,
    responses(
        (status = 201, description = "Platforms added successfully", body = Vec<PlatformRes>),
    )
)]
async fn add_dispatched_platforms(
    State(info): State<Arc<Mutex<Info>>>,
    Json(platform_add): Json<PlatformAdd>,
) -> Json<Vec<PlatformRes>> {
    log::info!("add_dispatched_platforms: platform_add={:?}", platform_add);
    let info = info.lock().unwrap();
    let mut dispatched_info = info.dispatched_info.lock().unwrap();
    let platform_info = info.platform_info.lock().unwrap();

    let mut added: Vec<PlatformRes> = Vec::new();
    for platform_id in &platform_add.platform_ids {
        if let Some(platform_res) = platform_info.get_platform(platform_id) {
            dispatched_info.add_platform_id(platform_id);
            added.push(platform_res.clone());
        } else {
            log::debug!("Platform not found, so not dispatched: {platform_id}");
        }
    }
    Json(added)
}

/// Response of a delete request.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDeleteRes {
    platform_id: String,
}

/// Delete a dispatched platform.
#[utoipa::path(
    delete,
    path = "/dispatched/platforms/{platform_id}",
    params(
        ("platform_id" = String, Path, description = "Platform ID"),
    ),
    responses(
       (status = 200, description = "Information of dispatched platform", body = PlatformDeleteRes)
    )
)]
async fn delete_dispatched_platform(
    State(info): State<Arc<Mutex<Info>>>,
    Path(platform_id): Path<String>,
) -> impl IntoResponse {
    log::info!("delete_dispatched_platform: platform_id={}", platform_id);
    let info = info.lock().unwrap();
    let mut dispatched_info = info.dispatched_info.lock().unwrap();

    match dispatched_info.delete_platform_id(&platform_id) {
        Some(_) => Json(PlatformDeleteRes { platform_id }).into_response(),
        None => (StatusCode::NOT_FOUND, "Platform not found").into_response(),
    }
}
