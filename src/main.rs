#![forbid(unsafe_code)]

use opto_sync_desktop_core::{app::DesktopApp, config::DesktopConfig};

fn main() {
    let cfg = DesktopConfig::from_env();
    DesktopApp::new(cfg).run();
}

