use crate::config;
use crate::http;

use chrono::{DateTime, SecondsFormat, Utc};
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

/// How many positions a request for the latest ones gets when it does not say.
/// The clients all say; this is for the CLI and for an ad hoc request.
const DEFAULT_LIMIT: u32 = 10;

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

/// What a caller wants of a platform's positions. Each variant carries a lower bound, or
/// says how to derive one: ODSS answers a request without one with the platform's complete
/// track, and takes tens of seconds over it for some platforms.
#[derive(Debug, Clone)]
pub enum PositionsRequest {
    /// The latest positions as of `end_date`, or as of now when it is not given.
    /// Only those within `defaultPositionsWindow` of that moment are considered.
    Latest {
        limit: Option<u32>,
        end_date: Option<String>,
    },

    /// The positions over a period the caller delimited.
    Period {
        limit: Option<u32>,
        start_date: String,
        end_date: Option<String>,
    },

    /// The positions since the given moment, for the dispatcher, which knows what it
    /// reported last. `None` for a platform it has not reported anything for yet.
    Since { time_ms: Option<u64> },
}

/// Get the latest positions for the given platform according to the default number of fixes.
pub async fn get_positions_per_config(platform_id: &str) -> Result<PositionsResponse, FetchError> {
    get_positions(
        platform_id,
        PositionsRequest::Latest {
            limit: None,
            end_date: None,
        },
    )
    .await
}

/// Maps the contract documented on the `/trackdb/platforms/{platform_id}/positions`
/// endpoint onto the ODSS API, which does not offer it directly:
///
/// - Every request carries a `startDate`, derived when the caller did not give one.
///   Without it ODSS answers with the platform's complete track, and it is also its
///   slowest query: some platforms take 33-60s to report their latest 20 positions that
///   way, against 0.1s for the same platforms with a date bound.
/// - `lastNumberOfFixes` is never sent, since ODSS drops the dates when it is given. A
///   limit that goes with a date is applied here instead.
/// - A period the caller delimited is sent with `numberOfPoints`, which ODSS applies as a
///   stride over the rows the query returned, and only when they outnumber it, so a period
///   holding fewer positions than that comes back whole. It is also sent with
///   `taperAccuracy`, which leaves the most recent third of the points unstrided. Such a
///   response is not truncated here: the excess would come off the older end, leaving the
///   period short of its own start.
pub async fn get_positions(
    platform_id: &str,
    request: PositionsRequest,
) -> Result<PositionsResponse, FetchError> {
    log::debug!("get_positions: platform_id='{platform_id}' request={request:?}");
    let odss_request = create_odss_request(config::get_config(), platform_id, &request, Utc::now());

    let endpoint = "/tracks";
    let track_res =
        make_get_request_with_params::<TrackRes>(endpoint, &odss_request.params).await?;
    track_res_to_positions_response(platform_id, odss_request.truncate_to, track_res)
}

/// A request to the ODSS `/tracks` endpoint, and what remains to be applied to its
/// response here.
struct OdssRequest<'a> {
    params: Vec<(&'a str, String)>,
    /// The limit ODSS could not be asked to apply, if any.
    truncate_to: Option<u32>,
}

fn create_odss_request<'a>(
    config: &config::Config,
    platform_id: &str,
    request: &PositionsRequest,
    now: DateTime<Utc>,
) -> OdssRequest<'a> {
    let mut params = vec![
        ("platformID", platform_id.to_string()),
        ("returnFormat", "json".to_string()),
        ("returnSRS", "4326".to_string()),
    ];

    let truncate_to = match request {
        PositionsRequest::Latest { limit, end_date } => {
            let upper = end_date.as_deref().and_then(parse_date).unwrap_or(now);
            params.push((
                "startDate",
                format_date(upper - to_chrono(config.default_positions_window)),
            ));
            if let Some(end_date) = end_date {
                params.push(("endDate", end_date.clone()));
            }
            // ODSS can only limit by discarding the dates, so the limit is applied here:
            Some(limit.filter(|limit| *limit > 0).unwrap_or(DEFAULT_LIMIT))
        }

        PositionsRequest::Period {
            limit,
            start_date,
            end_date,
        } => {
            params.push(("startDate", start_date.clone()));
            if let Some(end_date) = end_date {
                params.push(("endDate", end_date.clone()));
            }
            let number_of_points = limit
                .filter(|limit| *limit > 0)
                .unwrap_or(config.max_number_of_points)
                .min(config.max_number_of_points);
            params.push(("numberOfPoints", number_of_points.to_string()));
            params.push(("taperAccuracy", "true".to_string()));
            None
        }

        PositionsRequest::Since { time_ms } => {
            // Never look further back than the catch-up window: after an outage the
            // dispatcher would otherwise replay every position since it last ran.
            let floor = now - to_chrono(config.dispatch_catch_up_window);
            let since = time_ms
                .and_then(|time_ms| DateTime::from_timestamp_millis(time_ms as i64))
                .unwrap_or(floor)
                .clamp(floor, now);
            // ODSS takes the lower bound as exclusive, so the position at `since`,
            // already dispatched, is not asked for again.
            params.push(("startDate", format_date(since)));
            None
        }
    };

    OdssRequest {
        params,
        truncate_to,
    }
}

fn to_chrono(duration: Duration) -> chrono::Duration {
    chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::MAX)
}

fn format_date(date: DateTime<Utc>) -> String {
    date.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// `None` if the date cannot be parsed, so that the request falls back to `now` rather
/// than going out unbounded.
fn parse_date(date: &str) -> Option<DateTime<Utc>> {
    match DateTime::parse_from_rfc3339(date) {
        Ok(date) => Some(date.with_timezone(&Utc)),
        Err(e) => {
            log::warn!("could not parse date='{date}': {e}");
            None
        }
    }
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

    fn config() -> Config {
        Config {
            default_positions_window: Duration::from_secs(7 * 24 * 60 * 60),
            dispatch_catch_up_window: Duration::from_secs(60 * 60),
            max_number_of_points: 2000,
            ..Config::default()
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-20T18:00:00.000Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// The parameters beyond the ones fixed for every request, and what is left to apply
    /// to the response.
    fn request(request: PositionsRequest) -> (Vec<(String, String)>, Option<u32>) {
        let odss_request = create_odss_request(&config(), PLATFORM_ID, &request, now());
        let params = odss_request
            .params
            .into_iter()
            .filter(|(k, _)| !matches!(*k, "platformID" | "returnFormat" | "returnSRS"))
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        (params, odss_request.truncate_to)
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn latest_positions_as_of_now_get_a_derived_lower_bound() {
        let (params, truncate_to) = request(PositionsRequest::Latest {
            limit: Some(20),
            end_date: None,
        });
        assert_eq!(params, pairs(&[("startDate", "2026-09-13T18:00:00.000Z")]));
        // ODSS cannot apply the limit without dropping the dates:
        assert_eq!(truncate_to, Some(20));
    }

    #[test]
    fn latest_positions_as_of_an_upper_bound_derive_it_from_that_bound() {
        let (params, truncate_to) = request(PositionsRequest::Latest {
            limit: None,
            end_date: Some("2026-09-19T22:00:00.000Z".to_string()),
        });
        assert_eq!(
            params,
            pairs(&[
                ("startDate", "2026-09-12T22:00:00.000Z"),
                ("endDate", "2026-09-19T22:00:00.000Z"),
            ])
        );
        // no limit given, so the default applies:
        assert_eq!(truncate_to, Some(DEFAULT_LIMIT));
    }

    #[test]
    fn an_unparsable_upper_bound_falls_back_to_now_rather_than_going_unbounded() {
        let (params, _) = request(PositionsRequest::Latest {
            limit: Some(20),
            end_date: Some("not-a-date".to_string()),
        });
        assert_eq!(
            params,
            pairs(&[
                ("startDate", "2026-09-13T18:00:00.000Z"),
                ("endDate", "not-a-date"),
            ])
        );
    }

    /// A period the caller delimited, of whatever length and however far in the past:
    /// ODSS is asked for as many points as the request wants, tapered so that the most
    /// recent third of them is not strided.
    #[test]
    fn a_period_is_passed_with_number_of_points() {
        let (params, truncate_to) = request(PositionsRequest::Period {
            limit: Some(1001),
            start_date: "2025-09-19T00:00:00.000Z".to_string(),
            end_date: Some("2026-09-19T00:00:00.000Z".to_string()),
        });
        assert_eq!(
            params,
            pairs(&[
                ("startDate", "2025-09-19T00:00:00.000Z"),
                ("endDate", "2026-09-19T00:00:00.000Z"),
                ("numberOfPoints", "1001"),
                ("taperAccuracy", "true"),
            ])
        );
        // ODSS applied the limit as a stride over the period; truncating it here would
        // cut the period short at its older end:
        assert_eq!(truncate_to, None);
    }

    #[test]
    fn number_of_points_is_capped() {
        let (params, _) = request(PositionsRequest::Period {
            limit: Some(999_999),
            start_date: "2025-09-19T00:00:00.000Z".to_string(),
            end_date: None,
        });
        assert_eq!(
            params,
            pairs(&[
                ("startDate", "2025-09-19T00:00:00.000Z"),
                ("numberOfPoints", "2000"),
                ("taperAccuracy", "true"),
            ])
        );
    }

    #[test]
    fn a_dispatch_resumes_from_the_last_position_reported() {
        // 20 minutes before `now`, so within the catch up window:
        let (params, truncate_to) = request(PositionsRequest::Since {
            time_ms: Some(1789926000000),
        });
        assert_eq!(params, pairs(&[("startDate", "2026-09-20T17:40:00.000Z")]));
        // everything since then is new, and is to be dispatched:
        assert_eq!(truncate_to, None);
    }

    #[test]
    fn a_dispatch_does_not_look_further_back_than_the_dispatch_catch_up_window() {
        // last reported a week ago, as after an outage:
        let (params, _) = request(PositionsRequest::Since {
            time_ms: Some(1789322400000),
        });
        assert_eq!(params, pairs(&[("startDate", "2026-09-20T17:00:00.000Z")]));
    }

    #[test]
    fn a_dispatch_for_an_unreported_platform_starts_at_the_dispatch_catch_up_window() {
        let (params, _) = request(PositionsRequest::Since { time_ms: None });
        assert_eq!(params, pairs(&[("startDate", "2026-09-20T17:00:00.000Z")]));
    }

    /// A timestamp ahead of now, from a clock skew or a bad position, would otherwise
    /// pin the platform's lower bound in the future and stop it dispatching for good.
    #[test]
    fn a_dispatch_never_starts_in_the_future() {
        let (params, _) = request(PositionsRequest::Since {
            time_ms: Some(1789928400000),
        });
        assert_eq!(params, pairs(&[("startDate", "2026-09-20T18:00:00.000Z")]));
    }
}
