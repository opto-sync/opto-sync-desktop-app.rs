#![forbid(unsafe_code)]

use crate::lifecycle::SyncLifecycleSnapshot;
use crate::state::DesktopState;

/// Native UI surface. Not a webview and not React.
pub fn render(state: &DesktopState) -> String {
    format!(
        "Opto Sync desktop\nendpoint={}\nconnected={}\n",
        state.endpoint, state.connected
    )
}

/// Render lifecycle data only from the authoritative machine snapshot.
pub fn render_with_lifecycle(state: &DesktopState, lifecycle: SyncLifecycleSnapshot) -> String {
    format!(
        "{}lifecycle={}\ngeneration={}\n",
        render(state),
        lifecycle.phase_label(),
        lifecycle.generation,
    )
}
