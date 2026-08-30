#![forbid(unsafe_code)]

/// Phase projection of the canonical Opto-Sync mobile/desktop Quint model.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncLifecyclePhase {
    Idle,
    Acquiring,
    Running,
    Releasing,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncLifecycleEvent {
    Wake,
    Join,
    BeginAcquire,
    AcquireGranted,
    AcquireDeferred,
    Cancel,
    CycleSettled,
    ReleaseSettled,
    Close,
    ProcessAbort,
}

impl SyncLifecycleEvent {
    #[must_use]
    pub const fn requires_generation(self) -> bool {
        match self {
            Self::Wake => false,
            Self::Join => false,
            Self::BeginAcquire => false,
            Self::AcquireGranted => true,
            Self::AcquireDeferred => true,
            Self::Cancel => false,
            Self::CycleSettled => true,
            Self::ReleaseSettled => true,
            Self::Close => false,
            Self::ProcessAbort => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionDisposition {
    Applied,
    Rejected,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncLifecycleCommand {
    pub event: SyncLifecycleEvent,
    pub generation: Option<u64>,
}

impl SyncLifecycleCommand {
    #[must_use]
    pub const fn new(event: SyncLifecycleEvent) -> Self {
        Self {
            event,
            generation: None,
        }
    }

    #[must_use]
    pub const fn generated(event: SyncLifecycleEvent, generation: u64) -> Self {
        Self {
            event,
            generation: Some(generation),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SyncLifecycleSnapshot {
    pub phase: SyncLifecyclePhase,
    pub wake_pending: bool,
    pub close_requested: bool,
    pub cancel_requested: bool,
    pub permit_held: bool,
    pub generation: u64,
}

impl SyncLifecycleSnapshot {
    pub const INITIAL: Self = Self {
        phase: SyncLifecyclePhase::Idle,
        wake_pending: false,
        close_requested: false,
        cancel_requested: false,
        permit_held: false,
        generation: 0,
    };

    #[must_use]
    pub const fn is_valid(self) -> bool {
        let active_permit = match self.phase {
            SyncLifecyclePhase::Idle => false,
            SyncLifecyclePhase::Acquiring => false,
            SyncLifecyclePhase::Running => true,
            SyncLifecyclePhase::Releasing => true,
            SyncLifecyclePhase::Closed => false,
        };
        if self.permit_held != active_permit {
            return false;
        }
        if matches!(self.phase, SyncLifecyclePhase::Closed) {
            return self.close_requested
                && !self.wake_pending
                && !self.cancel_requested
                && !self.permit_held;
        }
        if self.close_requested && self.wake_pending {
            return false;
        }
        !self.cancel_requested || matches!(self.phase, SyncLifecyclePhase::Running)
    }

    /// Only this derived capability authorizes queue or network work.
    #[must_use]
    pub const fn may_run_sync_work(self) -> bool {
        matches!(self.phase, SyncLifecyclePhase::Running)
            && self.permit_held
            && !self.cancel_requested
            && !self.close_requested
    }

    #[must_use]
    pub const fn accepts_wake(self) -> bool {
        !matches!(self.phase, SyncLifecyclePhase::Closed) && !self.close_requested
    }

    #[must_use]
    pub const fn phase_label(self) -> &'static str {
        match self.phase {
            SyncLifecyclePhase::Idle => "Idle",
            SyncLifecyclePhase::Acquiring => "Acquiring ownership",
            SyncLifecyclePhase::Running => "Synchronizing",
            SyncLifecyclePhase::Releasing => "Releasing ownership",
            SyncLifecyclePhase::Closed => "Closed",
        }
    }
}

impl Default for SyncLifecycleSnapshot {
    fn default() -> Self {
        Self::INITIAL
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncLifecycleTransition {
    pub disposition: TransitionDisposition,
    pub before: SyncLifecycleSnapshot,
    pub after: SyncLifecycleSnapshot,
    pub command: SyncLifecycleCommand,
}

impl SyncLifecycleTransition {
    #[must_use]
    pub const fn applied(self) -> bool {
        matches!(self.disposition, TransitionDisposition::Applied)
    }
}

/// The only mutable lifecycle authority in the native app shell.
///
/// The reducer is total. Undefined transitions fail closed, and asynchronous
/// settlements from an older generation stutter without mutating the snapshot.
#[derive(Debug, Default)]
pub struct SyncLifecycleMachine {
    state: SyncLifecycleSnapshot,
}

impl SyncLifecycleMachine {
    #[must_use]
    pub const fn state(&self) -> SyncLifecycleSnapshot {
        self.state
    }

    pub fn dispatch(&mut self, command: SyncLifecycleCommand) -> SyncLifecycleTransition {
        let decision = Self::reduce(self.state, command);
        if decision.applied() {
            self.state = decision.after;
        }
        decision
    }

    #[must_use]
    pub fn reduce(
        state: SyncLifecycleSnapshot,
        command: SyncLifecycleCommand,
    ) -> SyncLifecycleTransition {
        let unchanged = |disposition| SyncLifecycleTransition {
            disposition,
            before: state,
            after: state,
            command,
        };

        if !state.is_valid() {
            return unchanged(TransitionDisposition::Rejected);
        }
        if command.event.requires_generation() {
            let Some(generation) = command.generation else {
                return unchanged(TransitionDisposition::Rejected);
            };
            if generation != state.generation {
                return unchanged(TransitionDisposition::Stale);
            }
        }

        let Some(next) = Self::next(state, command.event) else {
            return unchanged(TransitionDisposition::Rejected);
        };
        if !next.is_valid() {
            return unchanged(TransitionDisposition::Rejected);
        }
        SyncLifecycleTransition {
            disposition: TransitionDisposition::Applied,
            before: state,
            after: next,
            command,
        }
    }

    fn next(
        state: SyncLifecycleSnapshot,
        event: SyncLifecycleEvent,
    ) -> Option<SyncLifecycleSnapshot> {
        let mut next = state;
        match event {
            SyncLifecycleEvent::Wake => {
                if !state.accepts_wake() {
                    return None;
                }
                next.wake_pending = true;
            }
            SyncLifecycleEvent::Join => {
                let can_join = match state.phase {
                    SyncLifecyclePhase::Idle => false,
                    SyncLifecyclePhase::Acquiring => true,
                    SyncLifecyclePhase::Running => true,
                    SyncLifecyclePhase::Releasing => true,
                    SyncLifecyclePhase::Closed => false,
                };
                if !can_join {
                    return None;
                }
            }
            SyncLifecycleEvent::BeginAcquire => {
                if !matches!(state.phase, SyncLifecyclePhase::Idle)
                    || !state.wake_pending
                    || state.close_requested
                {
                    return None;
                }
                next.phase = SyncLifecyclePhase::Acquiring;
                next.wake_pending = false;
                next.generation = state.generation.checked_add(1)?;
            }
            SyncLifecycleEvent::AcquireGranted => {
                if !matches!(state.phase, SyncLifecyclePhase::Acquiring) {
                    return None;
                }
                next.phase = if state.close_requested {
                    SyncLifecyclePhase::Releasing
                } else {
                    SyncLifecyclePhase::Running
                };
                next.permit_held = true;
                next.cancel_requested = false;
            }
            SyncLifecycleEvent::AcquireDeferred => {
                if !matches!(state.phase, SyncLifecyclePhase::Acquiring) {
                    return None;
                }
                next.phase = if state.close_requested {
                    SyncLifecyclePhase::Closed
                } else {
                    SyncLifecyclePhase::Idle
                };
                if state.close_requested {
                    next.wake_pending = false;
                }
                next.cancel_requested = false;
                next.permit_held = false;
            }
            SyncLifecycleEvent::Cancel => {
                if !matches!(state.phase, SyncLifecyclePhase::Running) {
                    return None;
                }
                next.cancel_requested = true;
            }
            SyncLifecycleEvent::CycleSettled => {
                if !matches!(state.phase, SyncLifecyclePhase::Running) || !state.permit_held {
                    return None;
                }
                next.phase = SyncLifecyclePhase::Releasing;
                next.cancel_requested = false;
            }
            SyncLifecycleEvent::ReleaseSettled => {
                if !matches!(state.phase, SyncLifecyclePhase::Releasing) || !state.permit_held {
                    return None;
                }
                next.phase = if state.close_requested {
                    SyncLifecyclePhase::Closed
                } else {
                    SyncLifecyclePhase::Idle
                };
                if state.close_requested {
                    next.wake_pending = false;
                }
                next.cancel_requested = false;
                next.permit_held = false;
            }
            SyncLifecycleEvent::Close => {
                if matches!(state.phase, SyncLifecyclePhase::Closed) {
                    return None;
                }
                next.wake_pending = false;
                next.close_requested = true;
                if matches!(state.phase, SyncLifecyclePhase::Idle) {
                    next.phase = SyncLifecyclePhase::Closed;
                    next.cancel_requested = false;
                } else {
                    next.cancel_requested = matches!(state.phase, SyncLifecyclePhase::Running);
                }
            }
            SyncLifecycleEvent::ProcessAbort => {
                let can_abort = match state.phase {
                    SyncLifecyclePhase::Idle => false,
                    SyncLifecyclePhase::Acquiring => true,
                    SyncLifecyclePhase::Running => true,
                    SyncLifecyclePhase::Releasing => true,
                    SyncLifecyclePhase::Closed => false,
                };
                if !can_abort {
                    return None;
                }
                next.phase = if state.close_requested {
                    SyncLifecyclePhase::Closed
                } else {
                    SyncLifecyclePhase::Idle
                };
                next.wake_pending = false;
                next.cancel_requested = false;
                next.permit_held = false;
            }
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SyncLifecycleCommand, SyncLifecycleEvent, SyncLifecycleMachine, SyncLifecyclePhase,
        SyncLifecycleSnapshot, TransitionDisposition,
    };

    const PHASES: [SyncLifecyclePhase; 5] = [
        SyncLifecyclePhase::Idle,
        SyncLifecyclePhase::Acquiring,
        SyncLifecyclePhase::Running,
        SyncLifecyclePhase::Releasing,
        SyncLifecyclePhase::Closed,
    ];
    const EVENTS: [SyncLifecycleEvent; 10] = [
        SyncLifecycleEvent::Wake,
        SyncLifecycleEvent::Join,
        SyncLifecycleEvent::BeginAcquire,
        SyncLifecycleEvent::AcquireGranted,
        SyncLifecycleEvent::AcquireDeferred,
        SyncLifecycleEvent::Cancel,
        SyncLifecycleEvent::CycleSettled,
        SyncLifecycleEvent::ReleaseSettled,
        SyncLifecycleEvent::Close,
        SyncLifecycleEvent::ProcessAbort,
    ];

    fn apply(
        machine: &mut SyncLifecycleMachine,
        event: SyncLifecycleEvent,
        generation: Option<u64>,
    ) -> super::SyncLifecycleTransition {
        machine.dispatch(SyncLifecycleCommand { event, generation })
    }

    #[test]
    fn happy_path_is_valid_and_close_is_terminal() {
        let mut machine = SyncLifecycleMachine::default();
        assert!(apply(&mut machine, SyncLifecycleEvent::Wake, None).applied());
        let begin = apply(&mut machine, SyncLifecycleEvent::BeginAcquire, None);
        let generation = begin.after.generation;
        assert_eq!(generation, 1);
        assert!(apply(
            &mut machine,
            SyncLifecycleEvent::AcquireGranted,
            Some(generation)
        )
        .applied());
        assert!(machine.state().may_run_sync_work());
        assert!(apply(
            &mut machine,
            SyncLifecycleEvent::CycleSettled,
            Some(generation)
        )
        .applied());
        assert!(apply(
            &mut machine,
            SyncLifecycleEvent::ReleaseSettled,
            Some(generation)
        )
        .applied());
        assert!(apply(&mut machine, SyncLifecycleEvent::Close, None).applied());
        assert_eq!(machine.state().phase, SyncLifecyclePhase::Closed);

        let before = machine.state();
        let rejected = apply(&mut machine, SyncLifecycleEvent::Wake, None);
        assert_eq!(rejected.disposition, TransitionDisposition::Rejected);
        assert_eq!(machine.state(), before);
    }

    #[test]
    fn stale_completion_stutters() {
        let mut machine = SyncLifecycleMachine::default();
        apply(&mut machine, SyncLifecycleEvent::Wake, None);
        let begin = apply(&mut machine, SyncLifecycleEvent::BeginAcquire, None);
        let before = machine.state();
        let stale = apply(
            &mut machine,
            SyncLifecycleEvent::AcquireGranted,
            Some(begin.after.generation - 1),
        );
        assert_eq!(stale.disposition, TransitionDisposition::Stale);
        assert_eq!(stale.before, before);
        assert_eq!(stale.after, before);
        assert_eq!(machine.state(), before);
    }

    #[test]
    fn trailing_wake_preserves_active_generation() {
        let mut machine = SyncLifecycleMachine::default();
        apply(&mut machine, SyncLifecycleEvent::Wake, None);
        let begin = apply(&mut machine, SyncLifecycleEvent::BeginAcquire, None);
        let generation = begin.after.generation;
        apply(
            &mut machine,
            SyncLifecycleEvent::AcquireGranted,
            Some(generation),
        );
        apply(&mut machine, SyncLifecycleEvent::Wake, None);
        assert!(machine.state().wake_pending);
        assert_eq!(machine.state().generation, generation);
        apply(
            &mut machine,
            SyncLifecycleEvent::CycleSettled,
            Some(generation),
        );
        apply(
            &mut machine,
            SyncLifecycleEvent::ReleaseSettled,
            Some(generation),
        );
        let next = apply(&mut machine, SyncLifecycleEvent::BeginAcquire, None);
        assert_eq!(next.after.generation, generation + 1);
    }

    #[test]
    fn reducer_is_total_over_finite_input_space() {
        let mut examined = 0;
        for phase in PHASES {
            for wake_pending in [false, true] {
                for close_requested in [false, true] {
                    for cancel_requested in [false, true] {
                        for permit_held in [false, true] {
                            for generation in [0_u64, 1_u64] {
                                let state = SyncLifecycleSnapshot {
                                    phase,
                                    wake_pending,
                                    close_requested,
                                    cancel_requested,
                                    permit_held,
                                    generation,
                                };
                                for event in EVENTS {
                                    for command_generation in
                                        [None, Some(generation), Some(generation + 1)]
                                    {
                                        let decision = SyncLifecycleMachine::reduce(
                                            state,
                                            SyncLifecycleCommand {
                                                event,
                                                generation: command_generation,
                                            },
                                        );
                                        examined += 1;
                                        assert_eq!(decision.before, state);
                                        match decision.disposition {
                                            TransitionDisposition::Applied => {
                                                assert!(state.is_valid());
                                                assert!(decision.after.is_valid());
                                            }
                                            TransitionDisposition::Rejected => {
                                                assert_eq!(decision.after, state);
                                            }
                                            TransitionDisposition::Stale => {
                                                assert!(event.requires_generation());
                                                assert_eq!(decision.after, state);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(examined, 4_800);
    }
}
