# App lifecycle assurance

`mobile_desktop_lifecycle.qnt` is byte-identical to the canonical Opto-Sync
mobile/desktop ownership model at
`opto-sync/opto-sync-clients@fdf4fad9e2e841f66ecee19fca3b408d5fa7fa4c`.
Its SHA-256 digest is
`c6cb526867779178e6fab89f1333113576764e3760c31314475d9b17207cd00d`.

The native reducer projects to the model's five phases, ten non-stuttering
events, and idle/stuttering behavior. Runtime generations are an additional
refinement guard: an asynchronous settlement must name the current generation,
and stale settlements leave the complete snapshot unchanged.

CI type-checks the model, performs 10,000 bounded simulations, and exhaustively
checks the finite state graph with TLC. Rust tests enumerate all representable
phase/boolean/generation/event inputs, require every reducer call to return a
classified result, and check that rejected or stale commands do not mutate
state. Clippy denies wildcard enum match arms so later variants break CI until
their semantics are explicit.

## Proof boundary

The finite model proves its declared ownership, close, wake, and cancellation
invariants. Its liveness properties depend on the environment eventually
settling acquisition, callback work, and release. The checks do not prove OS
scheduler delivery, network availability, callback termination, durable lease
correctness, or unrestricted liveness. Those remain in the canonical
Opto-Sync client and platform assurance lanes.

This repository is an application package rather than a published Opto-Sync
SDK. The change adds an internal app-shell authority plus additive public types;
it does not remove or alter the existing `DesktopApp`, `DesktopState`, `probe`,
or `render` contracts. No semantic-version bump is required.
