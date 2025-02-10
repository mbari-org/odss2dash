use crate::config;

use serde::{Deserialize, Serialize};
use std::time::Duration;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlatformRes {
    #[serde(rename(deserialize = "_id", serialize = "_id"))]
    pub _id: String,
    pub name: String,
    pub abbreviation: String,
    pub type_name: Option<String>,
    pub color: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct TrackRes {
    status: String,
    data: TrackDataRes,
}

#[derive(Serialize, Deserialize, Debug)]
struct TrackDataRes {
    #[serde(rename(deserialize = "type"))]
    _type: String, // normally "LineString", which we simply assume.
    pub timestamps: Vec<u64>,
    pub coordinates: Vec<Vec<f64>>,
}

fn create_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

fn get_url(endpoint: &str) -> String {
    let config = config::get_config();
    format!("{}{endpoint}", config.odss_api)
}

fn make_get_request<T>(endpoint: &str) -> Option<T>
where
    T: std::fmt::Debug + for<'de> serde::Deserialize<'de>,
{
    make_get_request_with_params(endpoint, &Vec::new())
}

fn make_get_request_with_params<'a, T>(endpoint: &str, params: &Vec<(&'a str, String)>) -> Option<T>
where
    T: std::fmt::Debug + for<'de> serde::Deserialize<'de>,
{
    let log_prefix = || {
        let params = if params.is_empty() {
            "".to_string()
        } else {
            format!(
                "({})",
                params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
        format!("GET {endpoint}{params}")
    };

    log::debug!("{}", log_prefix());
    let url = get_url(endpoint);
    let req = create_agent().get(&url).query_pairs(params.clone());

    let mut response = match req.call() {
        Ok(res) => res,
        Err(e) => {
            log::error!("{}: request failed: {}", log_prefix(), e);
            return None;
        }
    };
    let res = match response.body_mut().read_json::<T>() {
        Ok(parsed) => parsed,
        Err(e) => {
            log::error!("{}: failed to parse response JSON: {}", log_prefix(), e);
            return None;
        }
    };
    log::debug!("GET {endpoint} => {:?}", res);
    Some(res)
}

pub fn get_platforms() -> Vec<PlatformRes> {
    let endpoint = "/platforms";
    make_get_request(endpoint).unwrap_or_else(Vec::new)
}

pub fn get_platform(platform_id: &str) -> Option<PlatformRes> {
    let endpoint = format!("/platforms/{platform_id}");
    make_get_request(&endpoint)
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PositionsResponse {
    pub platform_id: String,
    pub platform_name: Option<String>,
    pub positions: Vec<Position>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub time_ms: u64,
    pub lat: f64,
    pub lon: f64,
}

/// Get the latest positions for the given platform according to the default number of fixes.
pub fn get_positions_per_config(platform_id: &str) -> Option<PositionsResponse> {
    get_positions(platform_id, None, None, None)
}

/// Note that lastNumberOfFixes has precedence over, and in fact is mutually exclusive with
/// startDate/endDate in the ODSS implementation. Here, we give precedence to startDate/endDate
/// to facilitate playback in the Dash. That is, lastNumberOfFixes will not be passed to the
/// request to ODSS if any of startDate or endDate is given. In this case, this function will
/// truncate to the lastNumberOfFixes limit on the response.
///
pub fn get_positions(
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Option<PositionsResponse> {
    log::debug!("get_positions: platform_id='{}'", platform_id);
    let params =
        create_params_for_positions(platform_id, last_number_of_fixes, start_date, end_date);
    let endpoint = "/tracks";
    if let Some(track_res) = make_get_request_with_params::<TrackRes>(endpoint, &params) {
        track_res_to_positions_response(platform_id, last_number_of_fixes, track_res)
    } else {
        None
    }
}

fn create_params_for_positions<'a>(
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Vec<(&'a str, String)> {
    let config = config::get_config();

    let mut params = Vec::new();
    params.push(("platformID", platform_id.to_string()));
    params.push(("returnFormat", "json".to_string()));
    params.push(("returnSRS", "4326".to_string()));

    if start_date.is_some() || end_date.is_some() {
        if let Some(start_date) = start_date {
            params.push(("startDate", start_date));
        }
        if let Some(end_date) = end_date {
            params.push(("endDate", end_date));
        }
    } else {
        let default = config.default_last_number_of_fixes;
        let last_number_of_fixes = match last_number_of_fixes {
            Some(number) => {
                if number > 0 {
                    number
                } else {
                    default
                }
            }
            None => default,
        };
        params.push(("lastNumberOfFixes", last_number_of_fixes.to_string()));
    }
    params
}

fn track_res_to_positions_response(
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    track_res: TrackRes,
) -> Option<PositionsResponse> {
    log::debug!("odss track_res = {:?}", track_res);
    if track_res.status != "success" {
        log::error!(
            "GET response ok but with status != 'success'. track_res={:?}",
            track_res
        );
        return None;
    };
    let timestamps = track_res.data.timestamps;
    let coordinates = track_res.data.coordinates;
    let pairs = timestamps.iter().zip(coordinates.iter());
    let mut positions = pairs
        .map(|(time_ms, coords)| Position {
            time_ms: *time_ms,
            lat: coords[1],
            lon: coords[0],
        })
        .collect::<Vec<Position>>();

    // If given, apply lastNumberOfFixes restriction:
    if let Some(number) = last_number_of_fixes {
        positions.truncate(number as usize);
    }

    Some(PositionsResponse {
        platform_id: platform_id.to_string(),
        platform_name: None,
        positions,
    })
    // platform_name None here but will be set when incorporating platform info cache.
}
