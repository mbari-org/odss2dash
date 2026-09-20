use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::sync::OnceLock;
use std::time::Duration;

const CONFIG_FILENAME: &str = "odss2dash.toml";

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Get the configuration from the default file.
pub fn load_config() -> &'static Config {
    CONFIG.get_or_init(|| parse_config_file(CONFIG_FILENAME))
}

// allows directly setting the configuration for testing
#[cfg(test)]
pub fn set_config(config: Config) {
    CONFIG
        .set(config)
        .unwrap_or_else(|_| panic!("Configuration already set!"));
}

pub fn get_config() -> &'static Config {
    CONFIG
        .get()
        .unwrap_or_else(|| panic!("Configuration not set. Call load_config() first."))
}

/// Configuration for the odss2dash service. See `odds2dash.toml`.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub odss_api: String,
    pub external_url: String,
    pub port: u16,
    #[serde(deserialize_with = "humantime_serde::deserialize")]
    pub poll_period: Duration,

    /// Timeout for the requests to the ODSS API.
    /// Keep it below the timeout of the clients (Dash4 uses 15s), so that a slow
    /// ODSS is reported by this service instead of the client just giving up.
    #[serde(
        default = "default_odss_timeout",
        deserialize_with = "humantime_serde::deserialize"
    )]
    pub odss_timeout: Duration,

    /// Timeout for a POST to a TethysDash instance. An instance that does not answer
    /// is left alone for the rest of the dispatch cycle, so this is paid at most once
    /// per instance per cycle.
    #[serde(
        default = "default_tethysdash_timeout",
        deserialize_with = "humantime_serde::deserialize"
    )]
    pub tethysdash_timeout: Duration,

    /// For the requests polled by the clients: how far back a request for the latest
    /// positions looks. Such a request is not delimiting a period, but asking for the
    /// latest positions as of a moment, and left without a lower bound it would make ODSS
    /// return the complete track of the platform (tens of MB for some of them), so this
    /// window is used to derive one.
    #[serde(
        default = "default_positions_window",
        deserialize_with = "humantime_serde::deserialize"
    )]
    pub default_positions_window: Duration,

    /// How many platforms a dispatch cycle handles at a time. ODSS answers 8 of these
    /// requests at once about as fast as it answers two, and a cycle is a hundred or so
    /// of them, so there is no reason to wait for each before starting the next. Lower it
    /// to be gentler on ODSS.
    #[serde(default = "default_dispatch_concurrency")]
    pub dispatch_concurrency: usize,

    /// For the real time dispatch: how far back the dispatcher may look when it resumes.
    /// Its requests start from the position it last reported, but never from further back
    /// than this, so that coming back from an outage does not replay everything that
    /// happened meanwhile.
    #[serde(
        default = "default_dispatch_catch_up_window",
        deserialize_with = "humantime_serde::deserialize"
    )]
    pub dispatch_catch_up_window: Duration,

    /// Ceiling for the `numberOfPoints` requested to ODSS when a positions request
    /// delimits an explicit period, that is, for the number of positions ODSS returns
    /// when striding the track over that period.
    #[serde(default = "default_max_number_of_points")]
    pub max_number_of_points: u32,

    pub tethysdashes: Vec<TethysDashConfig>,
}

fn default_odss_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_tethysdash_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_positions_window() -> Duration {
    Duration::from_secs(7 * 24 * 60 * 60)
}

fn default_dispatch_concurrency() -> usize {
    8
}

fn default_dispatch_catch_up_window() -> Duration {
    Duration::from_hours(1)
}

fn default_max_number_of_points() -> u32 {
    20000
}

impl Default for Config {
    fn default() -> Self {
        Config {
            odss_api: String::new(),
            external_url: String::new(),
            port: 0,
            poll_period: Duration::from_secs(30),
            odss_timeout: default_odss_timeout(),
            tethysdash_timeout: default_tethysdash_timeout(),
            default_positions_window: default_positions_window(),
            dispatch_concurrency: default_dispatch_concurrency(),
            dispatch_catch_up_window: default_dispatch_catch_up_window(),
            max_number_of_points: default_max_number_of_points(),
            tethysdashes: Vec::new(),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TethysDashConfig {
    pub name: String,
    pub api: String,
    #[serde(deserialize_with = "api_key_deserialize")]
    pub api_key: String,
}

/// To process environment variables in the configuration file.
fn api_key_deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let api_key = String::deserialize(deserializer)?;
    if api_key.is_empty() {
        return Err(serde::de::Error::custom("api key is empty"));
    }
    if let Some(env_var) = api_key.strip_prefix('$') {
        match std::env::var(env_var) {
            Ok(val) => Ok(val),
            Err(_) => {
                log::warn!("'{}' undefined as environment variable.", env_var);
                Ok(api_key) // use as given
            }
        }
    } else {
        Ok(api_key)
    }
}

impl Config {
    pub fn redacted(&self) -> Config {
        let config = self.clone();
        Config {
            tethysdashes: config
                .tethysdashes
                .into_iter()
                .map(|td| TethysDashConfig {
                    api_key: "REDACTED".to_string(),
                    ..td
                })
                .collect(),
            ..config
        }
    }

    pub fn json_string(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

/// Parse the configuration from a file.
fn parse_config_file(filename: &str) -> Config {
    dotenv().ok();
    let toml_content =
        fs::read_to_string(filename).unwrap_or_else(|_| panic!("Failed to load {filename}"));
    parse_config_string(&toml_content).unwrap_or_else(|_| panic!("Failed to parse {filename}"))
}

/// Parse the configuration from a string.
fn parse_config_string(toml_content: &str) -> Result<Config, Box<dyn Error>> {
    Ok(toml::from_str(toml_content)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_config_string() {
        let toml_content = r#"
        odssApi = "https://odss.mbari.org/odss"
        externalUrl = "https://okeanids.mbari.org/odss2dash"
        port = 8080
        pollPeriod = "1 minute"
        defaultLastNumberOfFixes = 20
        [[tethysdashes]]
        name = "tethystest"
        api = "https://foo.example.net/TethysDash/api"
        apiKey = "eyFooBaz"
        "#;

        let config: Config = parse_config_string(toml_content).unwrap();

        assert_eq!(config.odss_api, "https://odss.mbari.org/odss");
        assert_eq!(config.external_url, "https://okeanids.mbari.org/odss2dash");
        assert_eq!(config.port, 8080);
        assert_eq!(config.poll_period, Duration::from_secs(60));
        // not given in the file, so the defaults apply:
        assert_eq!(config.odss_timeout, Duration::from_secs(10));
        assert_eq!(config.tethysdash_timeout, Duration::from_secs(10));
        assert_eq!(
            config.default_positions_window,
            Duration::from_secs(7 * 24 * 60 * 60)
        );
        assert_eq!(config.dispatch_concurrency, 8);
        assert_eq!(config.dispatch_catch_up_window, Duration::from_hours(1));
        assert_eq!(config.max_number_of_points, 20000);
        assert_eq!(
            config.tethysdashes,
            vec![TethysDashConfig {
                name: String::from("tethystest"),
                api: String::from("https://foo.example.net/TethysDash/api"),
                api_key: String::from("eyFooBaz")
            },]
        );
    }

    #[test]
    fn test_parse_config_file() {
        std::env::set_var("OKEANIDS_APIKEY", "eyFoo");
        std::env::set_var("TETHYSTEST_APIKEY", "eyBaz");

        let config = parse_config_file("src/test/odss2dash.toml");

        assert_eq!(config.odss_api, "https://odss.mbari.org/odss");
        assert_eq!(config.external_url, "http://localhost:3033");
        assert_eq!(config.port, 3033);
        assert_eq!(config.poll_period, Duration::from_secs(30));
        assert_eq!(config.odss_timeout, Duration::from_secs(20));
        assert_eq!(config.tethysdash_timeout, Duration::from_secs(5));
        assert_eq!(
            config.default_positions_window,
            Duration::from_secs(2 * 24 * 60 * 60)
        );
        assert_eq!(config.dispatch_concurrency, 3);
        assert_eq!(
            config.dispatch_catch_up_window,
            Duration::from_secs(15 * 60)
        );
        assert_eq!(config.max_number_of_points, 2000);
        assert_eq!(
            config.tethysdashes,
            vec![
                TethysDashConfig {
                    name: String::from("okeanids"),
                    api: String::from("https://okeanids.mbari.org/TethysDash/api"),
                    api_key: String::from("eyFoo")
                },
                TethysDashConfig {
                    name: String::from("tethystest"),
                    api: String::from("http://tethystest.shore.mbari.org/TethysDash/api"),
                    api_key: String::from("eyBaz")
                },
            ]
        );
    }
}
