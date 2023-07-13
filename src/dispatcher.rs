use crate::config;
use crate::dispatched_info::DispatchedInfo;
use crate::platform_info::PlatformInfo;
use crate::publisher::{PostXEventFn, Publisher};
use crate::tethysdash_client::XEvent;
use crate::trackdb_client::{self, PlatformRes, Position};

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type PlatformId = String;
type LastTsReported = u64;

type ReportedMap = BTreeMap<PlatformId, LastTsReported>;

pub struct Dispatcher {
    poll_period: Duration,
    publisher: Publisher,
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
}

/// File that keeps track of last notified position timestamps.
const REPORTED_PATH: &str = "./reported.json";

impl Dispatcher {
    pub fn new(
        post_xevent: PostXEventFn,
        platform_info: Arc<Mutex<PlatformInfo>>,
        dispatched_info: Arc<Mutex<DispatchedInfo>>,
    ) -> Self {
        let config = config::get_config();
        Self {
            poll_period: config.poll_period,
            publisher: Publisher::new(post_xevent, config.tethysdashes.clone()),
            platform_info,
            dispatched_info,
        }
    }

    pub fn launch_dispatch(&self) {
        println!(
            "\nodss2dash dispatcher is running  (polling every {} secs)",
            self.poll_period.as_secs()
        );
        loop {
            let num_dispatched = self.dispatch_one();
            println!(
                "Dispatch done. {} positions dispatched.  Will poll again in {} secs",
                num_dispatched,
                self.poll_period.as_secs()
            );
            std::thread::sleep(self.poll_period);
        }
    }

    pub fn launch_one_dispatch(&self) {
        let num_dispatched = self.dispatch_one();
        println!("Dispatch done. {} positions dispatched.", num_dispatched);
    }

    fn dispatch_one(&self) -> usize {
        let dispatched_info = self.dispatched_info.lock().unwrap();

        println!(
            "\nDispatching any new positions for {} platforms",
            dispatched_info.get_num_platforms()
        );

        let mut num_dispatched = 0;
        let mut reported_map = load_reported();
        for platform_id in &dispatched_info.get_platform_ids() {
            let platform_info = self.platform_info.lock().unwrap();
            let platform_res = &platform_info.get_platform(platform_id);
            if let Some(platform_res) = platform_res {
                num_dispatched += self.dispatch_platform(&mut reported_map, platform_res);
            } else {
                eprintln!("No platform by id: {platform_id}");
            }
        }

        save_reported(&reported_map);
        num_dispatched
    }

    fn dispatch_platform(&self, reported_map: &mut ReportedMap, platform: &PlatformRes) -> usize {
        let pos_res = trackdb_client::get_positions_per_config(&platform._id);
        if let Some(pos_res) = pos_res {
            self.report_positions(reported_map, platform, pos_res.positions)
        } else {
            0
        }
    }

    fn report_positions(
        &self,
        reported_map: &mut ReportedMap,
        platform: &PlatformRes,
        positions: Vec<Position>,
    ) -> usize {
        let last_ts_reported = reported_map.get(&platform._id).unwrap_or(&0);

        let mut new_to_report = positions
            .into_iter()
            .filter(|p| p.time_ms > *last_ts_reported)
            .collect::<Vec<Position>>();

        if !new_to_report.is_empty() {
            new_to_report.sort_by(|a, b| a.time_ms.cmp(&b.time_ms));
            print!("    {} ({}): new positions ", platform.name, platform._id);
            std::io::stdout().flush().unwrap();

            for position in &new_to_report {
                print!(".");
                std::io::stdout().flush().unwrap();
                self.report_position(reported_map, platform, position);

                let new_last_ts_reported = position.time_ms;
                reported_map.insert(platform._id.clone(), new_last_ts_reported);
            }
            println!();
        }
        new_to_report.len()
    }

    fn report_position(
        &self,
        reported_map: &mut ReportedMap,
        platform: &PlatformRes,
        position: &Position,
    ) {
        //println!("report_position {}", position);
        let xevent = XEvent {
            asset_id: platform._id.clone(),
            asset_name: platform.name.clone(),
            position: position.clone(),
            type_name: platform.type_name.clone(),
            color: platform.color.clone(),
            icon_url: platform.icon_url.clone(),
        };
        if self.publisher.publish_xevent(xevent).is_ok() {
            let new_last_ts_reported = position.time_ms;
            reported_map.insert(platform._id.clone(), new_last_ts_reported);
        }
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
