use crate::config;
use crate::http;

use chrono::{DateTime, FixedOffset, SecondsFormat};
use serde::{Deserialize, Serialize};
use std::fmt;
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

/// Maximum size accepted for a response body from ODSS.
const MAX_RESPONSE_BYTES: usize = 30 * 1024 * 1024;

/// Why a request to ODSS did not produce a usable response.
#[derive(Debug)]
pub enum FetchError {
    /// ODSS did not respond within `odssTimeout`.
    Timeout,
    /// Anything else: transport error, unsuccessful status, oversized or unparsable body.
    Upstream(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FetchError::Timeout => write!(f, "ODSS did not respond in time"),
            FetchError::Upstream(msg) => write!(f, "{msg}"),
        }
    }
}

fn get_url(endpoint: &str) -> String {
    let config = config::get_config();
    format!("{}{endpoint}", config.odss_api)
}

async fn make_get_request<T>(endpoint: &str) -> Result<T, FetchError>
where
    T: std::fmt::Debug + for<'de> serde::Deserialize<'de>,
{
    make_get_request_with_params(endpoint, &Vec::new()).await
}

async fn make_get_request_with_params<'a, T>(
    endpoint: &str,
    params: &Vec<(&'a str, String)>,
) -> Result<T, FetchError>
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

    let request = http::client()
        .get(&url)
        .timeout(config::get_config().odss_timeout)
        .query(params);

    let response = match request.send().await {
        Ok(response) => match response.error_for_status() {
            Ok(response) => response,
            Err(e) => {
                log::error!("{}: unsuccessful response: {}", log_prefix(), e);
                return Err(FetchError::Upstream(format!("unsuccessful response: {e}")));
            }
        },
        Err(e) if e.is_timeout() => {
            log::error!("{}: timed out", log_prefix());
            return Err(FetchError::Timeout);
        }
        Err(e) => {
            log::error!("{}: request failed: {}", log_prefix(), e);
            return Err(FetchError::Upstream(format!("request failed: {e}")));
        }
    };

    let body = read_body_limited(response, &log_prefix).await?;
    // ODSS answers some unrecognized platform IDs with an empty body; treat it as null
    // so that a `T` of `Option<_>` resolves it to `None` instead of a parse error:
    let body: &[u8] = if body.iter().all(u8::is_ascii_whitespace) {
        b"null"
    } else {
        &body
    };

    let res = match serde_json::from_slice::<T>(body) {
        Ok(parsed) => parsed,
        Err(e) => {
            log::error!("{}: failed to parse response JSON: {}", log_prefix(), e);
            return Err(FetchError::Upstream(format!(
                "failed to parse response JSON: {e}"
            )));
        }
    };
    log::debug!("GET {endpoint} => {:?}", res);
    Ok(res)
}

/// Reads the response body, giving up if it grows beyond [`MAX_RESPONSE_BYTES`].
async fn read_body_limited(
    mut response: reqwest::Response,
    log_prefix: &impl Fn() -> String,
) -> Result<Vec<u8>, FetchError> {
    let mut body: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    log::error!(
                        "{}: response body is larger than the {MAX_RESPONSE_BYTES} byte limit",
                        log_prefix()
                    );
                    return Err(FetchError::Upstream(format!(
                        "response body is larger than the {MAX_RESPONSE_BYTES} byte limit"
                    )));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => return Ok(body),
            Err(e) if e.is_timeout() => {
                log::error!("{}: timed out while reading the response", log_prefix());
                return Err(FetchError::Timeout);
            }
            Err(e) => {
                log::error!("{}: failed to read response body: {}", log_prefix(), e);
                return Err(FetchError::Upstream(format!(
                    "failed to read response body: {e}"
                )));
            }
        }
    }
}

pub async fn get_platforms() -> Result<Vec<PlatformRes>, FetchError> {
    let endpoint = "/platforms";
    make_get_request(endpoint).await
}

/// `Ok(None)` if ODSS does not know the platform, which it reports with a
/// `null` (or empty) body rather than with a 404.
pub async fn get_platform(platform_id: &str) -> Result<Option<PlatformRes>, FetchError> {
    let endpoint = format!("/platforms/{platform_id}");
    make_get_request::<Option<PlatformRes>>(&endpoint).await
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
pub async fn get_positions_per_config(platform_id: &str) -> Result<PositionsResponse, FetchError> {
    get_positions(platform_id, None, None, None).await
}

/// Maps the contract documented on the `/trackdb/platforms/{platform_id}/positions`
/// endpoint onto the ODSS API, which does not offer it directly:
///
/// - `lastNumberOfFixes` is never sent along with the dates, which ODSS would drop.
/// - A period is sent with `numberOfPoints`, which ODSS applies as a stride over the rows
///   the query returned, and only when they outnumber it, so a period holding fewer
///   positions than that comes back whole. It is also sent with `taperAccuracy`, which
///   leaves the most recent third of the points unstrided. A strided response is not
///   truncated here: the excess would come off the older end, leaving the period short of
///   its own start.
/// - An upper bound alone would leave the request unbounded below, and ODSS would answer
///   with the platform's complete track, so a lower bound is derived from
///   `defaultPositionsWindow`. If it captures nothing, the latest positions are requested
///   instead, which is what ODSS answers when given no dates, and the upper bound is
///   applied here.
pub async fn get_positions(
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<PositionsResponse, FetchError> {
    log::debug!("get_positions: platform_id='{}'", platform_id);
    let config = config::get_config();

    let res = get_positions_in_window(
        platform_id,
        last_number_of_fixes,
        start_date.clone(),
        end_date.clone(),
        config.default_positions_window,
    )
    .await?;

    // Only a derived lower bound can leave out positions that the caller would have gotten
    // otherwise, so only then is a second request warranted:
    let derived_lower_bound = start_date.is_none() && end_date.is_some();
    if res.positions.is_empty() && derived_lower_bound {
        log::info!(
            "get_positions: no positions for platform_id={platform_id} in the last {}; \
             requesting the latest positions instead",
            humantime::format_duration(config.default_positions_window),
        );
        let mut res = get_positions_in_window(
            platform_id,
            last_number_of_fixes,
            None,
            None,
            config.default_positions_window,
        )
        .await?;
        // ODSS ignores the dates when lastNumberOfFixes is given, so the upper bound
        // the caller asked for has to be applied here:
        if let Some(end_ms) = end_date.as_deref().and_then(parse_time_ms) {
            res.positions.retain(|position| position.time_ms <= end_ms);
        }
        return Ok(res);
    }
    Ok(res)
}

async fn get_positions_in_window(
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    start_date: Option<String>,
    end_date: Option<String>,
    window: Duration,
) -> Result<PositionsResponse, FetchError> {
    let explicit_period = start_date.is_some();
    let params = create_params_for_positions(
        config::get_config(),
        platform_id,
        last_number_of_fixes,
        start_date,
        end_date,
        window,
    );
    let endpoint = "/tracks";
    let track_res = make_get_request_with_params::<TrackRes>(endpoint, &params).await?;
    // With an explicit period ODSS has already applied the limit, as a stride over the
    // whole period; truncating that here would cut the period short at its older end:
    let truncate_to = if explicit_period {
        None
    } else {
        last_number_of_fixes
    };
    track_res_to_positions_response(platform_id, truncate_to, track_res)
}

fn create_params_for_positions<'a>(
    config: &config::Config,
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    start_date: Option<String>,
    end_date: Option<String>,
    window: Duration,
) -> Vec<(&'a str, String)> {
    let mut params = Vec::new();
    params.push(("platformID", platform_id.to_string()));
    params.push(("returnFormat", "json".to_string()));
    params.push(("returnSRS", "4326".to_string()));

    if start_date.is_some() || end_date.is_some() {
        // Only a period the caller delimited justifies striding: with a derived lower
        // bound the caller wants the latest positions, which must stay consecutive.
        let explicit_period = start_date.is_some();

        let start_date = match (&start_date, &end_date) {
            // An upper bound on its own leaves the request unbounded below:
            (None, Some(end_date)) => derive_start_date(end_date, window),
            _ => start_date,
        };

        if let Some(start_date) = &start_date {
            params.push(("startDate", start_date.clone()));
        }
        if let Some(end_date) = &end_date {
            params.push(("endDate", end_date.clone()));
        }

        if explicit_period {
            let number_of_points = last_number_of_fixes
                .filter(|number| *number > 0)
                .unwrap_or(config.max_number_of_points)
                .min(config.max_number_of_points);
            params.push(("numberOfPoints", number_of_points.to_string()));
            // Keeps the most recent third of those points unstrided, so that the latest
            // positions arrive complete however long the period is. ODSS strides nothing
            // at all when the period holds fewer positions than asked for.
            params.push(("taperAccuracy", "true".to_string()));
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

fn parse_time_ms(date: &str) -> Option<u64> {
    match DateTime::<FixedOffset>::parse_from_rfc3339(date) {
        Ok(date) => u64::try_from(date.timestamp_millis()).ok(),
        Err(e) => {
            log::warn!("could not parse date='{date}': {e}");
            None
        }
    }
}

/// The lower bound `window` before the given upper bound.
/// `None` if the upper bound cannot be parsed, in which case the request is
/// left as given and ODSS gets to reject it.
fn derive_start_date(end_date: &str, window: Duration) -> Option<String> {
    let end = match DateTime::<FixedOffset>::parse_from_rfc3339(end_date) {
        Ok(end) => end,
        Err(e) => {
            log::warn!("could not parse endDate='{end_date}': {e}");
            return None;
        }
    };
    let window = match chrono::Duration::from_std(window) {
        Ok(window) => window,
        Err(e) => {
            log::warn!("unusable positions window {window:?}: {e}");
            return None;
        }
    };
    let start = end - window;
    Some(start.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn track_res_to_positions_response(
    platform_id: &str,
    last_number_of_fixes: Option<u32>,
    track_res: TrackRes,
) -> Result<PositionsResponse, FetchError> {
    log::debug!("odss track_res = {:?}", track_res);
    if track_res.status != "success" {
        log::error!(
            "GET response ok but with status != 'success'. track_res={:?}",
            track_res
        );
        return Err(FetchError::Upstream(format!(
            "unsuccessful track response: status='{}'",
            track_res.status
        )));
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

    Ok(PositionsResponse {
        platform_id: platform_id.to_string(),
        platform_name: None,
        positions,
    })
    // platform_name None here but will be set when incorporating platform info cache.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use pretty_assertions::assert_eq;

    const PLATFORM_ID: &str = "669811c71c6e31522c529a7f";
    const WINDOW: Duration = Duration::from_secs(7 * 24 * 60 * 60);

    fn config() -> Config {
        Config {
            default_last_number_of_fixes: 5,
            max_number_of_points: 2000,
            ..Config::default()
        }
    }

    /// The parameters beyond the ones fixed for every request.
    fn params(
        last_number_of_fixes: Option<u32>,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Vec<(String, String)> {
        create_params_for_positions(
            &config(),
            PLATFORM_ID,
            last_number_of_fixes,
            start_date.map(String::from),
            end_date.map(String::from),
            WINDOW,
        )
        .into_iter()
        .filter(|(k, _)| !matches!(*k, "platformID" | "returnFormat" | "returnSRS"))
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn no_dates_requests_the_last_number_of_fixes() {
        assert_eq!(
            params(Some(3), None, None),
            pairs(&[("lastNumberOfFixes", "3")])
        );
    }

    #[test]
    fn no_dates_and_no_limit_uses_the_configured_default() {
        assert_eq!(
            params(None, None, None),
            pairs(&[("lastNumberOfFixes", "5")])
        );
    }

    /// Only an upper bound: not a period, but a request for the latest positions as of
    /// that moment, which on its own would make ODSS return the complete track.
    #[test]
    fn an_upper_bound_alone_gets_a_derived_lower_bound() {
        assert_eq!(
            params(Some(1001), None, Some("2026-09-19T22:00:00.000Z")),
            pairs(&[
                ("startDate", "2026-09-12T22:00:00.000Z"),
                ("endDate", "2026-09-19T22:00:00.000Z"),
            ])
        );
    }

    /// An explicit period, of whatever length and however far in the past: ODSS is asked
    /// for as many points as the request wants, tapered so that the most recent third of
    /// them is not strided.
    #[test]
    fn an_explicit_period_is_passed_with_number_of_points() {
        assert_eq!(
            params(
                Some(1001),
                Some("2025-09-19T00:00:00.000Z"),
                Some("2026-09-19T00:00:00.000Z")
            ),
            pairs(&[
                ("startDate", "2025-09-19T00:00:00.000Z"),
                ("endDate", "2026-09-19T00:00:00.000Z"),
                ("numberOfPoints", "1001"),
                ("taperAccuracy", "true"),
            ])
        );
    }

    /// A short period needs no special case: ODSS strides nothing when the period holds
    /// fewer positions than asked for.
    #[test]
    fn a_short_period_is_passed_the_same_way() {
        assert_eq!(
            params(
                Some(1001),
                Some("2026-09-18T00:00:00.000Z"),
                Some("2026-09-19T00:00:00.000Z")
            ),
            pairs(&[
                ("startDate", "2026-09-18T00:00:00.000Z"),
                ("endDate", "2026-09-19T00:00:00.000Z"),
                ("numberOfPoints", "1001"),
                ("taperAccuracy", "true"),
            ])
        );
    }

    #[test]
    fn number_of_points_is_capped() {
        assert_eq!(
            params(Some(999_999), Some("2025-09-19T00:00:00.000Z"), None),
            pairs(&[
                ("startDate", "2025-09-19T00:00:00.000Z"),
                ("numberOfPoints", "2000"),
                ("taperAccuracy", "true"),
            ])
        );
    }

    #[test]
    fn an_unparsable_upper_bound_is_left_for_odss_to_reject() {
        assert_eq!(
            params(Some(10), None, Some("not-a-date")),
            pairs(&[("endDate", "not-a-date")])
        );
    }
}
