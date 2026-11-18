# Validation

Validated on Linux with Rust 1.95.0 on 2026-11-04, against the exact revisions
in `compatibility.json`.

| Check | Result |
|---|---|
| `cargo fmt --all --check` | see notes.md |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | see notes.md |
| `cargo test --locked --offline` | 39 runtime unit tests |
| `python3 tools/horizon.py stage` | pinned Horizon export |
| `python3 tools/horizon.py check` | staged binary check |
| `python3 tools/horizon.py test` | embedded `game::bhc::tests` |
| `python3 tools/horizon.py build` | fast client binary |
| `tools/e2e.sh` | in-game default and control runs |

The exact outputs are in [notes.md](notes.md). The template's
`tools/test_template.py` and `tools/test_stage.py` are generic scaffold tests;
the port's own ladder is the table above.

No Minecraft resources or player data in the repository are read or changed;
e2e runs use a scratch `HORIZON_HOME`/`XDG_*` under `/tmp/opencode`.

## 2026-11-09 — Horizon `1d63e0a`, both artifacts

Horizon `1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9` + horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`, every step through
`ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0`.

| Check | Result |
|---|---|
| `cargo fmt` + `--check`, `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings |
| `cargo test --locked --offline` | 41 passed |
| `horizon.py stage` / `check` / `test` / `build` | exit 0; 0 warnings; embedded 41 passed, 1240 filtered |
| `tools/package.sh` | `.hmod` 13,844 B `f82f21da…0ee15c2bb` (46 entries), `.hnmod` 17,180 B `2ea7d63d…460146` (12) |
| `tools/e2e.sh` | failed at this pin on a harness race (tick-deferred heal + a stray hostile hit); harness hardened and re-run pending |

Evidence: `ports/_shared/evidence/bhc/`.
`upstream_ready.py bhc` still blocks on the one-file-per-project registry rule.
