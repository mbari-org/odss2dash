use crate::dispatched_info::DispatchedInfo;
use crate::dispatcher::Dispatcher;
use crate::platform_info::PlatformInfo;
use crate::server;
use crate::tethysdash_client::post_xevent;
use crate::trackdb_client;
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

/// Runs a single dispatch if once is true, otherwise runs dispatch in a loop.
pub fn dispatch(once: bool) {
    let platform_info = create_platform_info();
    let dispatched_info = create_dispatched_info();
    let dispatcher = create_dispatcher(platform_info, dispatched_info);
    if once {
        dispatcher.launch_one_dispatch();
    } else {
        dispatcher.launch_dispatch(None);
    }
}

/// Serves the odss2dash service.
/// If no_dispatch is true, only the server is launched.
pub fn serve(no_dispatch: bool) {
    if no_dispatch {
        serve_only();
    } else {
        serve_and_dispatch();
    }
}

/// Initializes platform info cache via query to TrackingDB/ODSS.
fn create_platform_info() -> Arc<Mutex<PlatformInfo>> {
    let platform_info = Arc::new(Mutex::new(PlatformInfo::default()));
    let platforms_res = trackdb_client::get_platforms();
    if platforms_res.is_empty() {
        eprintln!("warning: no platforms returned from TrackingDB/ODSS");
    } else {
        platform_info
            .lock()
            .map(|mut platform_info| {
                println!(
                    "Initializing platform cache with {} platforms found in TrackingDB/ODSS",
                    platforms_res.len()
                );
                platform_info.set_platforms(platforms_res);
            })
            .unwrap_or_else(|e| {
                eprintln!("unexpected: failed to acquire platform_info lock: {}", e);
            })
    }
    platform_info
}

fn create_dispatched_info() -> Arc<Mutex<DispatchedInfo>> {
    Arc::new(Mutex::new(DispatchedInfo::new()))
}

fn create_dispatcher(
    platform_info: Arc<Mutex<PlatformInfo>>,
    dispatched_info: Arc<Mutex<DispatchedInfo>>,
) -> Dispatcher {
    Dispatcher::new(post_xevent, platform_info, dispatched_info)
}

fn serve_only() {
    let platform_info = create_platform_info();
    let dispatched_info = create_dispatched_info();
    server::launch_server(platform_info, dispatched_info, None);
}

fn serve_and_dispatch() {
    let platform_info = create_platform_info();
    let dispatched_info = create_dispatched_info();

    let (done_sender, done_receiver) = mpsc::channel();

    let server_handle = {
        let platform_info = Arc::clone(&platform_info);
        let dispatched_info = Arc::clone(&dispatched_info);
        thread::spawn(move || {
            server::launch_server(platform_info, dispatched_info, Some(done_sender));
        })
    };

    let dispatch_handle = {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            create_dispatcher(platform_info, dispatched_info).launch_dispatch(Some(done_receiver));
        })
    };

    server_handle.join().unwrap();
    dispatch_handle.join().unwrap();
}
