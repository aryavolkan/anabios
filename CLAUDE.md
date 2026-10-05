# CLAUDE.md

Guidance for Claude Code in this repository.

## Communication with the user

Explain things in ASD-STE100 (Simplified Technical English):

- Use short sentences (20 words or fewer). Put one topic in each sentence.
- Use the active voice and the present tense.
- Use simple words. Use one word for one meaning. Do not use slang or idioms.
- Use "must" for a requirement and "can" for a possibility.
- Use lists for steps and for items. Keep a paragraph to 6 sentences or fewer.
- Technical names (for example: codex, scenario, state hash) are permitted.

## Project

anabios is a deterministic agent-based evolution sandbox in Rust with a Godot
viewer and a WebAssembly front end. The README and `docs/` describe the
engine. The determinism contract is in `docs/determinism-contract.md`.

## Commands

```bash
cargo build --release -p anabios-headless
cargo test --workspace --lib                 # unit tests
cargo test --workspace --tests --release     # full gate, as in CI
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 1000
```

## Rules

- A run must stay bit-identical per seed. Read `docs/determinism-contract.md`
  before you touch serialized state or the RNG.
- Every `#[serde(skip)]` field must satisfy the three-category skip rule in that
  document.
- Do not change a scenario file without an update to `docs/scenarios.md`.
