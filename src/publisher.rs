use crate::config::TethysDashConfig;
use crate::tethysdash_client::XEvent;

pub type PostXEventFn = fn(&TethysDashConfig, XEvent) -> Result<(), String>;

pub struct Publisher {
    post_xevent: PostXEventFn,
    tethysdashes: Vec<TethysDashConfig>,
}

impl Publisher {
    pub fn new(post_xevent: PostXEventFn, tethysdashes: Vec<TethysDashConfig>) -> Publisher {
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
