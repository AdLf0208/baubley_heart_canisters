# Working on this Horizon native mod

This is a **flat SDK package** (mod API 0.2): `horizon-mod.toml` + `Cargo.toml` + `src/lib.rs`
(`impl mod_api::Hooks`) + `src/runtime.rs` (the pure rules and their tests) + `assets/` + `data/`.
Horizon retired the `.hnmod` container in the mod-API merge (#16941), so there is no
`compatibility.json`, `mixins/<id>/`, `horizon-mixin.toml`, plan/injection schema or
`rust-toolchain.toml` here, and none of them should be recreated. The retired tree is under
`../_shared/evidence/bhc/retired-hnmod-0.1.0/` for reading only.

> **Build slots (v2):** every build/test goes through the campaign runner
> (`ports/tools/build-slot.sh` from the campaign root). It **blocks in the kernel** until a slot
> is free, so **never poll and never background a build**; a timeout exits **75**. It does **not**
> `cd` into the port — wrap the command (`bash -c 'cd ports/<id> && …'`) and never pass
> `--target-dir`. Read `../_shared/build-slots-usage.md` before running anything.

```sh
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step test \
    -- bash -c 'cd ports/bhc && cargo test --offline'
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step build \
    -- bash -c 'cd ports/bhc && rm -f Cargo.lock && cargo horizon-mod build'
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step check \
    -- bash -c 'cd ports/bhc && cargo horizon-mod check bhc.hmod'
```

- `cargo horizon-mod` fetches the pinned SDK into `.horizon/sdk/` (gitignored, never packed).
  `build` packs the `.hmod`; it does **not** compile against the game — that is
  `horizon-launcher native-build . --instance <instance> --trust-native-code`.
- **`src/runtime.rs` is byte-identical to the retired bundle's `runtime.rs`** (SHA-256
  `0757718f…1b62`), apart from the 2026-11-20 rename of its local `Config` to `ServerConfig`
  (current SHA-256 `f7cc16dd…4218`) so `uses.json` stops recording it as the SDK `type:Config`;
  the behaviour rules live there, `src/lib.rs` only converts SDK types to theirs. New rule → a
  test in `runtime.rs`; new glue → a private fn and a test in `lib.rs`.
- Only the API the SDK publishes: `cargo horizon-mod hooks <text>` (or
  `../_shared/sdk/0b14f8a/surface.json`) finds the exact method; never guess a name, and record an
  API gap in `notes.md` instead of faking one. `use_duration`/`use_animation` are `ItemPart`
  methods, not global hooks.
- The delivered package is `bhc.hmod` **in the port root**, never in `target/`; `LICENSE` and
  `LICENSE-NOTICE.md` travel inside it.
- Keep the port separate from Horizon and horizon-mixins; never modify the pin checkouts, never
  commit binaries, exported sources or Minecraft assets, and never put Mojang files in the tree.
- `python3 ports/tools/check-skills.py --notes ports/bhc` must stay green, and `notes.md`
  records every claim with its command and output.
