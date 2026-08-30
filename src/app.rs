#![forbid(unsafe_code)]

use crate::config::DesktopConfig;
use crate::lifecycle::{
    SyncLifecycleCommand, SyncLifecycleEvent, SyncLifecycleMachine, TransitionDisposition,
};
use crate::net;
use crate::ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopAppError {
    LifecycleRejected {
        event: SyncLifecycleEvent,
        disposition: TransitionDisposition,
    },
    SyncWorkNotAuthorized,
}

impl std::fmt::Display for DesktopAppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LifecycleRejected { event, disposition } => write!(
                formatter,
                "lifecycle rejected {event:?} with disposition {disposition:?}"
            ),
            Self::SyncWorkNotAuthorized => {
                formatter.write_str("lifecycle did not authorize sync work")
            }
        }
    }
}

impl std::error::Error for DesktopAppError {}

pub struct DesktopApp {
    config: DesktopConfig,
}

impl DesktopApp {
    pub fn new(config: DesktopConfig) -> Self {
        Self { config }
    }

    pub fn run(&self) {
        match self.run_once() {
            Ok(output) => print!("{output}"),
            Err(error) => eprintln!("Opto Sync desktop failed closed: {error}"),
        }
    }

    /// Execute one probe only while the lifecycle machine owns a permit.
    pub fn run_once(&self) -> Result<String, DesktopAppError> {
        let mut lifecycle = SyncLifecycleMachine::default();
        apply(
            &mut lifecycle,
            SyncLifecycleCommand::new(SyncLifecycleEvent::Wake),
        )?;
        let begin = apply(
            &mut lifecycle,
            SyncLifecycleCommand::new(SyncLifecycleEvent::BeginAcquire),
        )?;
        let generation = begin.after.generation;
        apply(
            &mut lifecycle,
            SyncLifecycleCommand::generated(SyncLifecycleEvent::AcquireGranted, generation),
        )?;
        if !lifecycle.state().may_run_sync_work() {
            return Err(DesktopAppError::SyncWorkNotAuthorized);
        }

        let state = net::probe(&self.config.api_base);
        apply(
            &mut lifecycle,
            SyncLifecycleCommand::generated(SyncLifecycleEvent::CycleSettled, generation),
        )?;
        apply(
            &mut lifecycle,
            SyncLifecycleCommand::generated(SyncLifecycleEvent::ReleaseSettled, generation),
        )?;
        Ok(ui::render_with_lifecycle(&state, lifecycle.state()))
    }
}

fn apply(
    lifecycle: &mut SyncLifecycleMachine,
    command: SyncLifecycleCommand,
) -> Result<crate::lifecycle::SyncLifecycleTransition, DesktopAppError> {
    let decision = lifecycle.dispatch(command);
    if decision.applied() {
        Ok(decision)
    } else {
        Err(DesktopAppError::LifecycleRejected {
            event: command.event,
            disposition: decision.disposition,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::DesktopApp;
    use crate::config::DesktopConfig;

    #[test]
    fn run_once_uses_formal_lifecycle_path() {
        let app = DesktopApp::new(DesktopConfig {
            api_base: "https://sync.invalid".to_owned(),
        });
        let output = app.run_once().expect("modeled probe should complete");
        assert!(output.contains("endpoint=https://sync.invalid"));
        assert!(output.contains("lifecycle=Idle"));
        assert!(output.contains("generation=1"));
    }
}
