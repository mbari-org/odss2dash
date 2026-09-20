use crate::config::TethysDashConfig;
use crate::tethysdash_client::XEvent;

use crate::styles::Styles;

use base64::Engine;
use chrono::{DateTime, TimeDelta, Utc};

pub type PostXEventFn = fn(&TethysDashConfig, XEvent) -> Result<(), String>;

pub struct Publisher {
    post_xevent: PostXEventFn,
    tethysdashes: Vec<TethysDashConfig>,
}

impl Publisher {
    pub fn new(post_xevent: PostXEventFn, tethysdashes: Vec<TethysDashConfig>) -> Publisher {
        report_token_expirations(&tethysdashes);
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

/// Reports the API key (JWT) expiration for each configured TethysDash instance,
/// along with the rest of what the service prints as it starts.
fn report_token_expirations(tethysdashes: &[TethysDashConfig]) {
    if tethysdashes.is_empty() {
        return;
    }
    println!("\nTethysDash API key expirations:");
    let now = Utc::now();
    let styles = Styles::for_stdout();
    for td in tethysdashes {
        println!(
            "  - {:10} : {}",
            td.name,
            api_key_expiration(&td.api_key, now, &styles)
        );
    }
}

/// How an API key's expiration reads, both as the service starts and in `check-config`.
/// Colored when it goes to a terminal, so that a key needing attention stands out; the
/// escapes are left out otherwise, as when the output is a log file or `docker logs`.
pub(crate) fn api_key_expiration(api_key: &str, now: DateTime<Utc>, styles: &Styles) -> String {
    let soon = now + TimeDelta::days(30);
    match jwt_expiration(api_key) {
        Some(exp) if exp <= now => styles.error(format!(
            "EXPIRED {} ago at {}",
            format_time_delta(now - exp),
            exp.to_rfc3339()
        )),
        Some(exp) if exp <= soon => styles.warning(format!(
            "WARNING: expires soon, in {} at {}",
            format_time_delta(exp - now),
            exp.to_rfc3339()
        )),
        Some(exp) => format!(
            "expires in {} at {}",
            format_time_delta(exp - now),
            exp.to_rfc3339()
        ),
        None => "could not determine API key expiration".to_string(),
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

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-20T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// JWT with the given `exp`, unsigned.
    fn jwt(exp: i64) -> String {
        use base64::Engine;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{{\"exp\":{exp}}}"));
        format!("eyJhbGciOiJIUzI1NiJ9.{payload}.sig")
    }

    #[test]
    fn api_key_expiration_reports_a_key_with_time_left() {
        // 2027-01-01
        let summary = api_key_expiration(&jwt(1798761600), now(), &Styles::plain());
        assert_eq!(
            summary,
            "expires in 103 day(s) at 2027-01-01T00:00:00+00:00"
        );
    }

    #[test]
    fn api_key_expiration_warns_about_one_expiring_soon() {
        // 10 days out, within the 30-day warning
        let summary = api_key_expiration(&jwt(1790726400), now(), &Styles::plain());
        assert_eq!(
            summary,
            "WARNING: expires soon, in 10 day(s) at 2026-09-30T00:00:00+00:00"
        );
    }

    #[test]
    fn api_key_expiration_reports_an_expired_key() {
        // 2020-09-15
        let summary = api_key_expiration(&jwt(1600128000), now(), &Styles::plain());
        assert_eq!(
            summary,
            "EXPIRED 2196 day(s) ago at 2020-09-15T00:00:00+00:00"
        );
    }

    #[test]
    fn api_key_expiration_says_so_when_it_cannot_tell() {
        assert_eq!(
            api_key_expiration("not-a-jwt", now(), &Styles::plain()),
            "could not determine API key expiration"
        );
    }

    #[test]
    fn only_a_key_needing_attention_is_colored() {
        let styles = Styles::colored();

        let expired = api_key_expiration(&jwt(1600128000), now(), &styles);
        assert!(
            expired.starts_with("\u{1b}[1m\u{1b}[31mEXPIRED") && expired.ends_with("\u{1b}[0m"),
            "unexpected rendering: {expired:?}"
        );

        let soon = api_key_expiration(&jwt(1790726400), now(), &styles);
        assert!(
            soon.starts_with("\u{1b}[33mWARNING") && soon.ends_with("\u{1b}[0m"),
            "unexpected rendering: {soon:?}"
        );

        // nothing to draw attention to, so no escapes at all:
        let fine = api_key_expiration(&jwt(1798761600), now(), &styles);
        assert!(!fine.contains('\u{1b}'), "unexpected rendering: {fine:?}");
    }

    #[test]
    fn test_jwt_expiration_malformed() {
        assert!(jwt_expiration("not-a-jwt").is_none());
    }
}
