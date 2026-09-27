#![forbid(unsafe_code)]

use crate::state::DesktopState;
use opto_sync_desktop_fenced_lifecycle::{SyncLifecyclePhase, SyncLifecycleSnapshot};

/// Native UI surface. Not a webview and not React.
pub fn render(state: &DesktopState) -> String {
    format!(
        "Opto Sync desktop\nendpoint={}\nconnected={}\n",
        state.endpoint, state.connected
    )
}

/// Render lifecycle data from the shared machine snapshot. Phase labels remain
/// presentation policy owned by the desktop application.
pub fn render_with_lifecycle(state: &DesktopState, lifecycle: SyncLifecycleSnapshot) -> String {
    format!(
        "{}lifecycle={}\ngeneration={}\n",
        render(state),
        phase_label(lifecycle.lifecycle.phase),
        lifecycle.generation,
    )
}

const fn phase_label(phase: SyncLifecyclePhase) -> &'static str {
    match phase {
        SyncLifecyclePhase::Idle => "Idle",
        SyncLifecyclePhase::Acquiring => "Acquiring ownership",
        SyncLifecyclePhase::Running => "Synchronizing",
        SyncLifecyclePhase::Releasing => "Releasing ownership",
        SyncLifecyclePhase::Closed => "Closed",
    }
}
