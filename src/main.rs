#![forbid(unsafe_code)]

use opto_sync_desktop_core::{app::DesktopApp, config::DesktopConfig};

fn main() {
    // Shared lifecycle logging stays local and leaves stdout available for IPC.
    let _desktop_session = next_loggers::desktop::DesktopSession::start(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )
    .ok();

    let cfg = DesktopConfig::from_env();
    DesktopApp::new(cfg).run();
}
