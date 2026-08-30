# opto-sync-desktop-app.rs

Native Rust desktop app. No webviews, no React. UI rendering is isolated in `src/ui.rs`.

Sync ownership is controlled by one fail-closed lifecycle machine in
`src/lifecycle.rs`. Its phase/event projection is checked against the canonical
Quint model under `formal/`; Clippy rejects wildcard enum match arms so future
state or event variants require explicit semantics.
