# Baubley Heart Canisters — Horizon port (SDK format, 0.2.2)

A clean-room Rust rewrite of
[Baubley Heart Canisters 1.21.1-1.4.3](https://github.com/Traverse-Joe/Baubley-Heart-Canisters)
(MIT) for Horizon, targeted at Minecraft 26.3. It is one flat SDK package (mod API `0.2`) at the
SDK build `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` / horizon-mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.

Carried heart canisters raise the carrier's max health: **2 health points per canister**, with
each colour capped by `heart_stack_size` (default 10). The bonus is applied through the
`bhc:extra_health` max-health modifier and synced to the client, and it is re-applied every
player tick so respawns and dimension changes keep it. `config/bhc.conf` holds the cap.

The miniature hearts drop from mobs through `loot_drops` with upstream's default
entities/chances (red from hostile mobs at 15 %, yellow from bosses, green from the dragon, blue
from the warden), configurable per-colour chance, amount and an on/off switch; the world's
`/gamerule mob_drops` is honoured too, because the engine still dispatches the hook after it has
cleared the loot table; and they are
**eaten over 30 ticks** by one `ItemPart` on the four hearts, healing the tier's upstream amount
(red 20, yellow 40, green 60, blue 80 health points, clamped to max health), consuming one
unless the player is creative, and never granting permanent max health. `heart_eating` and the
four `*_heart_heal` keys tune it.

Upstream stores canisters in a Curios Heart Amulet; this slice defers Curios, the amulet and the
GUI, so the main inventory (plus off-hand) stands in for the amulet's slots. See
[notes.md](notes.md) for the upstream behaviour with `file:line`, the pin answers, gaps, the
migration recipe and verification evidence, and
[`../_shared/upstream/bhc/FINDINGS.md`](../_shared/upstream/bhc/FINDINGS.md) for the source
analysis.

**Local-only**: the licence (MIT, rank 0) allows derivatives, but the campaign publishes nothing
without the maintainer's approval (ARCHITECTURE.md §8).

## Layout

```text
horizon-mod.toml       the manifest and the only identity (id, version, api 0.2, [native] pins)
Cargo.toml             one crate; horizon-mod-api from .horizon/sdk/ and nothing else
src/lib.rs             the one impl mod_api::Hooks: config, per-player tick, heart finish, loot,
                       register; plus the MiniHeartPart ItemPart
src/runtime.rs         the pure health/drop/heart/config core + its 41 tests (the retired
                       bundle's rules; the type was renamed and two comments added, see notes.md)
assets/bhc/            item definitions, models, textures, lang
data/bhc/              item templates, creative tab, canister recipe
data/c/tags/           the two common tags the recipe names
tools/e2e.sh           in-game scenario (needs a launcher-built game; default + MODE=control)
notes.md               behaviour, design, gaps, migration, verification
```

The delivered package is `bhc.hmod` **in this directory**, with `LICENSE` and
`LICENSE-NOTICE.md` inside it; its size and SHA-256 are recorded in
[notes.md](notes.md). `target/` is disposable.

## Commands

```sh
# from the campaign root: the lease only; it does not cd into the port
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step test \
    -- bash -c 'cd ports/bhc && CARGO_INCREMENTAL=0 cargo test --offline'
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step lint \
    -- bash -c 'cd ports/bhc && cargo fmt --check &&
        CARGO_INCREMENTAL=0 cargo clippy --all-targets --offline -- -D warnings'
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step build \
    -- bash -c 'cd ports/bhc && rm -f Cargo.lock && cargo horizon-mod build'
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step check \
    -- bash -c 'cd ports/bhc && cargo horizon-mod check bhc.hmod &&
        cargo horizon-mod fits bhc.hmod --build nightly'
```

`cargo horizon-mod` fetches the pinned SDK into `.horizon/sdk/` (gitignored, never packed) and
packs the `.hmod`; it does not compile against the game — that is
`horizon-launcher native-build . --instance <instance> --trust-native-code`. `rm -f Cargo.lock`
before the build: a lock written by `cargo test` would be packed and change the bytes.

The in-game scenario is `tools/e2e.sh`, run with `GAME=<launcher-built horizon-client>`: the
default leg proves the canister health table (20→22→24→26→24→40→20), that 24 survives a respawn
and a relaunch, the mob-drop differential (a zombie drops the configured hearts, a cow none) and
the eat table (a red heart heals exactly 20 and is consumed, a stick heals nothing, creative
keeps its heart, a full-health survival use still consumes one, max health never rises from
eating). `MODE=control` sets `heart_stack_size = 0`, `mob_drops = false` and `heart_eating =
false` and proves the same actions change nothing.
