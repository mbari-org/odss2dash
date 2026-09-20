use crate::config;
use crate::dispatched_info::DispatchedInfo;
use crate::platform_info::PlatformInfo;
use crate::publisher::Publisher;
use crate::tethysdash_client::XEvent;
use crate::trackdb_client::{self, PlatformRes, Position, PositionsRequest};

use futures_util::stream::{self, StreamExt};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

type PlatformId = String;
type LastTsReported = u64;

type ReportedMap = BTreeMap<PlatformId, LastTsReported>;

pub struct Dispatcher {
    poll_period: Duration,
    /// How many platforms the cycle handles at a time; at least one.
    concurrency: usize,
    publisher: Publisher,
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
}

/// File that keeps track of last notified position timestamps.
const REPORTED_PATH: &str = "./reported.json";

/// What a dispatch cycle did about one platform.
struct PlatformOutcome {
    platform_id: PlatformId,
    /// The newest position reported, if any, to resume from next time.
    last_ts_reported: Option<LastTsReported>,
    num_dispatched: usize,
}

impl Dispatcher {
    pub fn new(
        platform_info: Arc<Mutex<PlatformInfo>>,
        dispatched_info: Arc<Mutex<DispatchedInfo>>,
    ) -> Self {
        let config = config::get_config();
        Self {
            poll_period: config.poll_period,
            concurrency: config.dispatch_concurrency.max(1),
            publisher: Publisher::new(config.tethysdashes.clone()),
            platform_info,
            dispatched_info,
        }
    }

    pub async fn launch_dispatch(&self, done_receiver: Option<mpsc::Receiver<()>>) {
        println!(
            "\nDispatcher is running  (polling every {} secs)",
            self.poll_period.as_secs()
        );

        // quickly react to done signal:
        let loop_sleep = Duration::from_millis(333);

        let mut remaining_time_to_dispatch = Duration::from_secs(0);
        loop {
            if let Some(done_receiver) = &done_receiver {
                match done_receiver.try_recv() {
                    Ok(_) => {
                        println!("\nDispatcher: received done signal.");
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // will happen if some error in server launching
                        println!("\nDispatcher: done signal disconnected.");
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }

            if remaining_time_to_dispatch.is_zero() {
                let num_dispatched = self.dispatch_one().await;
                println!(
                    "Dispatch done. {} positions dispatched.  Will poll again in {} secs",
                    num_dispatched,
                    self.poll_period.as_secs()
                );
                remaining_time_to_dispatch = self.poll_period;
            } else {
                if remaining_time_to_dispatch >= loop_sleep {
                    remaining_time_to_dispatch -= loop_sleep;
                } else {
                    remaining_time_to_dispatch = Duration::from_secs(0);
                }
                tokio::time::sleep(loop_sleep).await;
            }
        }
    }

    pub async fn launch_one_dispatch(&self) {
        let num_dispatched = self.dispatch_one().await;
        println!("Dispatch done. {} positions dispatched.", num_dispatched);
    }

    async fn dispatch_one(&self) -> usize {
        let platforms = self.get_dispatched_platforms();

        println!(
            "\nDispatching any new positions for {} platforms",
            platforms.len()
        );

        let mut reported_map = load_reported();
        self.publisher.begin_cycle();

        let outcomes: Vec<PlatformOutcome> = stream::iter(&platforms)
            .map(|platform| self.dispatch_platform(&reported_map, platform))
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        // The map is updated once the platforms are done with, so that each of them reads
        // the same starting point no matter the order they complete in:
        let mut num_dispatched = 0;
        for outcome in outcomes {
            num_dispatched += outcome.num_dispatched;
            if let Some(last_ts_reported) = outcome.last_ts_reported {
                reported_map.insert(outcome.platform_id, last_ts_reported);
            }
        }

        save_reported(&reported_map);
        num_dispatched
    }

    fn get_dispatched_platforms(&self) -> Vec<PlatformRes> {
        let dispatched_info = self.dispatched_info.lock().unwrap();
        let platform_info = self.platform_info.lock().unwrap();
        dispatched_info
            .get_platform_ids()
            .iter()
            .fold(vec![], |mut acc, id| {
                match platform_info.get_platform(id) {
                    Some(platform) => {
                        acc.push(platform);
                    }
                    _ => {
                        eprintln!("No platform by id: {id}");
                    }
                }
                acc
            })
    }

    async fn dispatch_platform(
        &self,
        reported_map: &ReportedMap,
        platform: &PlatformRes,
    ) -> PlatformOutcome {
        let last_ts_reported = reported_map.get(&platform._id).copied();

        // Ask only for what has happened since the last position reported for the
        // platform, which is also the only lower bound ODSS answers quickly:
        let request = PositionsRequest::Since {
            time_ms: last_ts_reported,
        };
        match trackdb_client::get_positions(&platform._id, request).await {
            Ok(pos_res) => {
                self.report_positions(last_ts_reported, platform, pos_res.positions)
                    .await
            }
            Err(e) => {
                log::error!(
                    "Error getting positions for platform {}: {e}",
                    platform.name
                );
                PlatformOutcome {
                    platform_id: platform._id.clone(),
                    last_ts_reported: None,
                    num_dispatched: 0,
                }
            }
        }
    }

    async fn report_positions(
        &self,
        last_ts_reported: Option<LastTsReported>,
        platform: &PlatformRes,
        positions: Vec<Position>,
    ) -> PlatformOutcome {
        let last_ts_reported = last_ts_reported.unwrap_or(0);

        let mut new_to_report = positions
            .into_iter()
            .filter(|p| p.time_ms > last_ts_reported)
            .collect::<Vec<Position>>();

        // Oldest first, so that the newest reported is also the last one, and an
        // interrupted cycle resumes without a gap:
        new_to_report.sort_by_key(|a| a.time_ms);

        let mut newest_reported = None;
        for position in &new_to_report {
            self.report_position(platform, position).await;
            newest_reported = Some(position.time_ms);
        }

        if !new_to_report.is_empty() {
            println!(
                "    {} ({}): {} new position(s) dispatched",
                platform.name,
                platform._id,
                new_to_report.len()
            );
        }

        PlatformOutcome {
            platform_id: platform._id.clone(),
            last_ts_reported: newest_reported,
            num_dispatched: new_to_report.len(),
        }
    }

    async fn report_position(&self, platform: &PlatformRes, position: &Position) {
        let xevent = XEvent {
            asset_id: platform._id.clone(),
            asset_name: platform.name.clone(),
            position: position.clone(),
            type_name: platform.type_name.clone(),
            color: platform.color.clone(),
            icon_url: platform.icon_url.clone(),
        };
        let _ = self.publisher.publish_xevent(xevent).await;
    }
}

fn load_reported() -> ReportedMap {
    let reported_map = if Path::new(REPORTED_PATH).exists() {
        let s = std::fs::read_to_string(REPORTED_PATH).unwrap();
        serde_json::from_str(&s).unwrap()
    } else {
        let new_reported = ReportedMap::new();
        save_reported(&new_reported);
        new_reported
    };
    log::debug!("{}", serde_json::to_string_pretty(&reported_map).unwrap());
    reported_map
}

fn save_reported(reported_map: &ReportedMap) {
    let f = File::create(REPORTED_PATH).unwrap();
    serde_json::to_writer_pretty(f, reported_map).unwrap();
}
