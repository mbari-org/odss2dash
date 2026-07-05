use crate::config::TethysDashConfig;
use crate::tethysdash_client::XEvent;

use base64::Engine;
use chrono::{DateTime, TimeDelta, Utc};

pub type PostXEventFn = fn(&TethysDashConfig, XEvent) -> Result<(), String>;

pub struct Publisher {
    post_xevent: PostXEventFn,
    tethysdashes: Vec<TethysDashConfig>,
}

impl Publisher {
    pub fn new(post_xevent: PostXEventFn, tethysdashes: Vec<TethysDashConfig>) -> Publisher {
        log_token_expirations(&tethysdashes);
        Publisher {
            post_xevent,
            tethysdashes,
        }
    }

    pub fn publish_xevent(&self, xevent: XEvent) -> Result<(), String> {
        let post_xevent = self.post_xevent;
        for tethysdash_config in &self.tethysdashes {
            match post_xevent(tethysdash_config, xevent.clone()) {
                Ok(_) => (),
                Err(e) => {
                    log::error!(
                        "Error posting XEvent to TethysDash instance '{}': {}",
                        tethysdash_config.name,
                        e
                    );
                }
            }
        }
        Ok(())
    }
}

/// Logs the API key (JWT) expiration for each configured TethysDash instance.
fn log_token_expirations(tethysdashes: &[TethysDashConfig]) {
    for td in tethysdashes {
        match jwt_expiration(&td.api_key) {
            Some(exp) => {
                let now = Utc::now();
                if exp > now {
                    log::info!(
                        "TethysDash instance '{}': API key expires at {} (in {})",
                        td.name,
                        exp.to_rfc3339(),
                        format_time_delta(exp - now)
                    );
                } else {
                    log::warn!(
                        "TethysDash instance '{}': API key EXPIRED at {} ({} ago)",
                        td.name,
                        exp.to_rfc3339(),
                        format_time_delta(now - exp)
                    );
                }
            }
            None => log::warn!(
                "TethysDash instance '{}': could not determine API key expiration",
                td.name
            ),
        }
    }
}

/// Extracts the `exp` claim from a JWT, without verifying its signature.
/// Returns `None` if the token is not a well-formed JWT with an `exp` claim.
pub(crate) fn jwt_expiration(jwt: &str) -> Option<DateTime<Utc>> {
    let payload = jwt.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let exp = claims.get("exp")?.as_i64()?;
    DateTime::from_timestamp(exp, 0)
}

/// Renders a time delta using its largest meaningful unit.
pub(crate) fn format_time_delta(delta: TimeDelta) -> String {
    let days = delta.num_days();
    if days > 0 {
        return format!("{days} day(s)");
    }
    let hours = delta.num_hours();
    if hours > 0 {
        return format!("{hours} hour(s)");
    }
    format!("{} minute(s)", delta.num_minutes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_expiration() {
        // JWT with payload {"exp": 1893456000} (2030-01-01T00:00:00Z), unsigned.
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjE4OTM0NTYwMDB9.sig";
        let exp = jwt_expiration(jwt).expect("should extract exp");
        assert_eq!(exp.timestamp(), 1893456000);
    }

    #[test]
    fn test_jwt_expiration_no_exp() {
        // Payload {"sub":"x"} has no exp claim.
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.sig";
        assert!(jwt_expiration(jwt).is_none());
    }

    #[test]
    fn test_jwt_expiration_malformed() {
        assert!(jwt_expiration("not-a-jwt").is_none());
    }
}
