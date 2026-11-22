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

## 2026-11-22 — 0.2.3 at the campaign pin `0a875da` (the melted-heart chain)

Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + horizon-mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, every cargo/launcher/e2e step through
`ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0` and no `--target-dir`.

| Check | Result |
|---|---|
| `cargo test --offline` | **45 passed**, 0 failed (41 runtime + 4 glue; no code changed) |
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings |
| `cargo horizon-mod build` ×2 (`rm -f Cargo.lock` each) | exit 0, `cmp`-identical |
| package `bhc.hmod` | **49,120 B**, SHA-256 `2788a8aed2b919abbbdd33278ed31e4629f40f83265af6376ab60457fa2ff3ae`, 84 entries, no packed `Cargo.lock` |
| `cargo horizon-mod check` / `fits --build 0a875da` / `fits --build 0b14f8a` | exit 0; `0a875da  made for`, `0b14f8a  fits` |
| `verify-sdk-package.py bhc` | **1/1** (`src/runtime.rs` byte-identical `a854a78601f5…`) |
| `tools/e2e.sh` (launcher-built game at `0a875da`) | **`PASS` + `EXIT:0`** |
| `MODE=control` / `SELFTEST=1` | `FAIL` + `EXIT:1` / `FAIL` + `EXIT:1` |
| the pre-content red run | `FAIL` + `EXIT:1` (`the package did not provide bhc:red_heart_melted`) |
| `check-e2e-logs.py bhc-e2e --strict` | `logs=11 ok=11 missing-exit=0 bad-exit=0` |

Evidence: `ports/_shared/evidence/bhc/sdk-0a875da-0.2.3/` and
`ports/_shared/evidence/bhc-e2e/e2e-0.2.3-*.log`. The transcript is in [notes.md](notes.md)
(§ `Slice 4 (0.2.3)`).

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
