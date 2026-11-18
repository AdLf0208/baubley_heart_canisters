# Baubley Heart Canisters — port notes

> **Format (2026-11-18): the port is now a flat SDK `.hmod`.** Horizon retired `.hnmod` in the
> mod-API merge (#16941, launcher ≥ 0.2.70); the bundle this file described below (PR #792,
> closed by the 2026-11-18 format purge) was migrated to `horizon-mod.toml` + `Cargo.toml` +
> `src/lib.rs` (`impl mod_api::Hooks`) + `src/runtime.rs` + `assets/`/`data/` at the SDK build
> **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** / horizon-mixins
> **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, mod API `0.2`, port version **0.2.0**. Read
> **`## Migration to the SDK format (2026-11-18, SDK 0b14f8a)`** at the end of this file first:
> it is the current state. Everything above it is the history of the retired bundle (old pins,
> `mixins/`, `horizon-mixin.toml`, the staging ladder) and is kept as the record, not as
> instructions — `compatibility.json`, `mixins/<id>/`, `tools/horizon.py` and the `.hnmod` no
> longer exist.

Ported from **Baubley Heart Canisters 1.21.1-1.4.3**
(`Traverse-Joe/Baubley-Heart-Canisters`, **MIT**) for Minecraft Java 1.21.1
NeoForge. Upstream read at commit
`f71dd3e06cb6c7cdf9741725103226f6d454c5bb` (branch `1.21-neoforge`,
2026-09-30); the full extraction with `file:line` evidence is in
[`../_shared/upstream/bhc/FINDINGS.md`](../_shared/upstream/bhc/FINDINGS.md).

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8).
The licence is MIT, so derivatives are allowed; publication still needs the
maintainer's approval.

Status: **slice 3 implemented and verified in-game** — the five canister
items plus the four edible miniature hearts, a pure 39-test core (health,
mob-drop and heart-use rules), five hook helpers, a valid `.hmod`,
`stage → check → test` green with 0 warnings, and a fail-capable The End e2e
covering the canister health table, the mob-drop differential and the heart
heal/consume table (default PASS + control PASS).

## Behaviour (slice 3): edible miniature hearts

The four miniature hearts are **consumable heals, not permanent hearts**.
`HeartType` gives each tier a `healAmount` (`HeartType.java:3-24`: red 20,
yellow 40, green 60, blue 80; soul 0) and `ItemHeart` uses `UseAnim.EAT` with
`getUseDuration` 30 ticks (`ItemHeart.java:38-52`). `finishUsingItem` calls
`entityLiving.heal(this.type.healAmount)`, shrinks the stack by one unless the
player is creative, and calls `ServerPlayer.resetSentInfo()` to force the
health sync (`ItemHeart.java:54-69`). It never touches
`Attributes.MAX_HEALTH`: the mod's only permanent-heart behaviour is the
canister/amulet formula (`HealthModifier.java:149-206`), which slice 1 already
carries. The hearts are also canister ingredients and smelt into melted hearts
(`BHCRecipeProvider.java:28-31,34-37,40`), but the melted-heart/relic-apple
chain is deferred, so no miniature-heart recipe is carried.

The port models the whole decision in `runtime.rs`:

| Piece | Port rule | Upstream |
|---|---|---|
| Heal amount | `Config::heart_heal(tier)`, defaults 20/40/60/80, clamped 0–1000 | `HeartType.java:4-8`, `ItemHeart.java:57` |
| Cap | `heal_health` adds the amount and clamps to the live max health | `LivingEntity.heal` |
| Consumption | one item unless the game mode is creative | `ItemHeart.java:59-61` |
| Permanent max health | never changed; `HeartUse` has no max-health field | `ItemHeart.java:55-68` (no attribute write) |
| Toggle | `heart_eating` (port addition, default true) | upstream has no switch |

The glue hooks `use_item` (API 13) and claims the click; it reads the acting
player's live max health and health (the hook swaps the acting player's state
in, guest or host, the same way the canister scan does), calls `use_heart`,
writes the new health through `PlayerMotion::set_health` (the pin's synced
health float), and shrinks the acting hand unless creative.
`MiniHeart::from_item` recognises exactly the four ids; canisters and
`bhc:canister` are not edible. When the acting player is a guest (the headless
local player included), the `server_tick` End hook resends that player's
inventory because the hub's per-tick slot diff misses an in-use decrement
(the akashictome finding at this pin).

**Deviation — instant use, not a 30-tick chew.** Upstream's effect runs in
`finishUsingItem` after a 30-tick `EAT` use (`ItemHeart.java:38-52,55-69`).
The pin's hook catalog (108 hooks at `c9d474d`) has neither `use_duration` nor
`living_entity_use_item_finish` (API 90 is later); the same gap is recorded for
solcarrot in [`../_shared/hook-feasibility.md`](../_shared/hook-feasibility.md).
The port therefore applies the heal on the `use_item` start and claims the
swing. The amount, the creative rule and the consumption are upstream's, but
there is no 1.5 s eating animation. Because the effect is immediate, the
solcarrot `player_tick` completion sampler (and its 0-tick blind spot) is not
used.

## Behaviour (slice 2): mob-drop miniature hearts

Upstream adds miniature hearts in `LivingDropsEvent` through
`DropHandler.onEntityDrop` and `getEntityDrops`: for each of red, yellow,
green and blue it rolls every entry of the `drops.json` map whose entity name
or category matches the dead mob, and adds one `new ItemStack(heart)` per
successful roll (`DropHandler.java:32-52,58-65,67-112,114-119`). The shipped
`drops.json` defaults are red from `hostile` at 15 %, yellow from `boss` at
100 %, green from `dragon` at 100 % and blue from `minecraft:warden` at 100 %
(`BaubleyHeartCanisters.java:52-82`, defaults `:75-78`).

The port reproduces those four default entries through the `loot_drops` hook
(`API 75`). The pin names the dead mob by kind, so the glue reads its entity
type's category (`monster`, `creature`, ...) and the pure core maps:

| Heart | Upstream entry | Port rule | Default chance |
|---|---|---|---|
| `bhc:red_heart` | `"hostile"` 0.15 | category `monster`, except the wither and the dragon; the warden still counts | 0.15 |
| `bhc:yellow_heart` | `"boss"` 1.0 | the boss set minus the dragon: wither and warden | 1.0 |
| `bhc:green_heart` | `"dragon"` 1.0 | `minecraft:ender_dragon` | 1.0 |
| `bhc:blue_heart` | `"minecraft:warden"` 1.0 | `minecraft:warden` | 1.0 |

The warden therefore matches red, yellow and blue, exactly as upstream's
separate entries do (`DropHandler.java:85-109`). Upstream's chance test is
`(int)(chance * 100)` and `random.nextInt(100) < percent`
(`DropHandler.java:114-119`); the port's `roll` takes the 0..100 draw as an
argument, so unit tests inject it. One successful entry adds one stack of
`heart_drop_amount` (upstream's `new ItemStack(heart)` is count 1).

The hook is filtered to a real mob death: `entity` names the mob, `used` is
false (shearing/brushing excluded) and `table` is non-empty (a block break
names no entity; a leashed mob's lead drop has no table). Block loot is never
touched. A missing entity-type entry yields an empty category and therefore
no configured group.

## Behaviour (slice 1)

- **Items**: `bhc:red_heart_canister`, `bhc:yellow_heart_canister`,
  `bhc:green_heart_canister`, `bhc:blue_heart_canister`,
  `bhc:soul_heart_canister`, plus the empty `bhc:canister`.
- **Carried count**: every canister stack in the player's main inventory and
  off hand is counted, per colour.
- **Bonus**: `2 × Σ min(count(colour), heart_stack_size)` health points, added
  through the `bhc:extra_health` `minecraft:max_health` modifier. All five
  colours count, including soul (upstream's soul amulet has a soul slot and
  `getHeartCount` sums every slot).
- **Health on change**: current health keeps its gap below max, exactly as
  upstream's `updatePlayerHealth`: a gain heals by the max-health delta, a loss
  hurts by it, clamped to `0..new_max`.
- **Reapply**: the modifier is refreshed on every player tick, so a respawn or
  dimension change re-applies it; the inventory is authoritative, so no file is
  needed.
- **Sync**: `PlayerMotion::set_modifier` feeds the pin's dirty-attribute sync
  (`take_dirty`/`lan/attributes`), and `set_health` writes the synced health
  float.

## Upstream semantics (`1.21.1-1.4.3`)

| Behaviour | Evidence |
|---|---|
| Five canister items, one per `HeartType` | `RegistryHandler.java:40-44` |
| Canister stack size = `heartStackSize` (default 10, range 0–99) | `BaseHeartCanister.java:10-13`, `ConfigHandler.java:27-29` |
| Plain amulet stores 4 colours; soul amulet stores 5 | `HeartAmuletContainer.java:11,17-21`, `SoulHeartAmuletContainer.java:11,17-21` |
| Canisters live in the amulet's `bhc:hearts` component | `RegistryHandler.java:87`, `InventoryUtil.java:10-12` |
| Bonus = `getHeartCount × 2` health points | `HealthModifier.java:161,184-206` |
| Modifier id `bhc:extra_health`, `ADD_VALUE` | `HealthModifier.java:19,171` |
| Health keeps the gap below max; ≤ 0 kills | `HealthModifier.java:155,172-181` |
| Sync forced with `resetSentInfo()` | `HealthModifier.java:175-177` |
| Equip/unequip path is Curios, not inventory | `ItemHeartAmulet.java:61-81`, `ItemSoulHeartAmulet.java:59-77` |
| Tier `healAmount`/`multiplier` are for eating hearts/belts, not canisters | `HeartType.java:3-24`, `ItemHeart.java:55-68` |
| Edible miniature hearts heal 20/40/60/80 by tier | `HeartType.java:4-8`, `ItemHeart.java:57` |
| Eating consumes one unless creative; no max-health write | `ItemHeart.java:57-65` |
| A heart's use is `EAT` for 30 ticks | `ItemHeart.java:38-52` |
| Heart recipes need the deferred melted hearts | `BHCRecipeProvider.java:28-31,40` |
| Optional starting-health base tweak (default off) | `ConfigHandler.java:73-86`, `StartingHealthHandler.java:16-22` |
| Empty `canister` recipe: iron + bones | `BHCRecipeProvider.java:62-63` |
| Creative tab `itemGroup.bhcTab` | `RegistryHandler.java:85`, `en_us.json` |
| Item names/textures/models are the MIT repository's own | `assets/bhc/lang/en_us.json`, `assets/bhc/textures/item/*_heart_canister.png`, `*_heart.png` |
| Four miniature heart items | `RegistryHandler.java:32-35` |
| Mobs drop hearts in `LivingDropsEvent` | `DropHandler.java:32-52` |
| Drop types are red/yellow/green/blue | `DropHandler.java:58-65` |
| Entity-name/category matching (`passive`, `hostile`, `boss`, `dragon`) | `DropHandler.java:67-112` |
| Default `drops.json` entries: red hostile 0.15, yellow boss 1.0, green dragon 1.0, blue warden 1.0 | `BaubleyHeartCanisters.java:52-82` (`:75-78`) |
| Chance is a truncated percent roll, one heart per hit | `DropHandler.java:114-119` |
| `drops.json` lives at `config/bhc/drops.json` | `BaubleyHeartCanisters.java:54-60` |

## Config (`config/bhc.conf`)

A missing file is written with the defaults:

```
# Baubley Heart Canisters (Horizon) — server settings.
# Maximum canisters of one colour that count, and the canister stack size upstream used.
heart_stack_size = 10
# Miniature hearts mobs drop (upstream's drops.json defaults).
mob_drops = true
heart_drop_amount = 1
red_heart_drop_chance = 0.15
yellow_heart_drop_chance = 1
green_heart_drop_chance = 1
blue_heart_drop_chance = 1
# Miniature hearts: eating heals these health points (upstream's HeartType).
heart_eating = true
red_heart_heal = 20
yellow_heart_heal = 40
green_heart_heal = 60
blue_heart_heal = 80
```

`heart_stack_size` is upstream's stack size and per-colour cap (clamped
0–99). `mob_drops` enables/disables slice 2's drop table; upstream has no such
switch (it is a port addition so a server can turn the table off, and the
control run uses it). `heart_drop_amount` is clamped 0–64 (upstream always
drops one). The four chances are clamped 0–1 and default to upstream's
`drops.json` values (`BaubleyHeartCanisters.java:75-78`); a missing or invalid
value keeps the default. `heart_eating` is a port switch; the four
`*_heart_heal` values default to `HeartType`'s 20/40/60/80 and are clamped
0–1000. Everything is read at `server_about_to_start`.

The item's declared stack size is the upstream default 10 (content is static);
a config value above 10 raises the health cap but not the item's stack limit
(recorded deviation). The full `drops.json` entity/category table is not
exposed: slice 2 carries upstream's shipped default entries, with config
chances and amount (documented subset).

## Hooks (pin `c9d474d`)

| Hook | Helper | What it does |
|---|---|---|
| `server_about_to_start` (v1, API 44) | `about.rs` | Read/create `config/bhc.conf`. |
| `player_tick` (v1, API 31) | `player.rs` | Scan the acting player's inventory/off hand, apply the modifier, keep health's gap. |
| `use_item` (v1, API 13) | `use_item.rs` | A right click with a miniature heart heals the acting player by its tier amount and consumes one unless creative. |
| `server_tick` (v1, API 4) | `server_tick.rs` | End of a running server tick: resend a guest's inventory after a heart use decremented the acting hand. |
| `loot_drops` (v1, API 75) | `loot.rs` | On a mob death's rolled loot, add the configured miniature hearts that match its drop group. |

The acting player comes from `native_acting_player()`. A LAN guest's inventory
is read from the hub's guest record (`game.lan`); the host's is `game.inventory`
plus `game.offhand`. Embedded tests run as `game::bhc::tests`.

## Content (`.hmod`)

No ATM10 jar was carried; everything comes from the **MIT upstream repository**:

- `data/bhc/horizon/item/*.json`: five canisters on the `minecraft:stick`
  template with `max_stack_size: 10`, the four miniature hearts and `canister`
  on the stick template.
- `assets/bhc/items/*.json` + `models/item/*.json` (parent
  `minecraft:item/generated`) and the real 16×16 upstream textures, including
  `*_heart.png`. Nothing was invented.
- `assets/bhc/lang/en_us.json`: the ten item names and `itemGroup.bhcTab`.
- `data/bhc/horizon/creative_tab/canisters.json`: the ten items.
- `data/bhc/recipe/canister.json`: upstream's vanilla-only empty-canister
  recipe (` I / I#I / I `, `#c:bones`, `#c:ingots/iron`), with
  `data/c/tags/item/bones.json` and `.../ingots/iron.json` supplied (26.3 ships
  no `c:` tags).

The four miniature hearts stay plain stick-template items (no
`food`/`consumable` component): the native `use_item` hook owns the heal, so
the vanilla food path never runs and a full-hunger player can still use one.
Upstream's heart recipes need the deferred `*_heart_melted` items
(`BHCRecipeProvider.java:28-31,40`), so none is carried.

## Deviations and gaps

1. **Inventory scan instead of Curios.** Upstream grants the bonus only while a
   Heart Amulet is equipped (`ItemHeartAmulet.java:61-81`). With Curios and the
   amulet deferred, the port's main-inventory/off-hand scan stands in for the
   amulet's colour slots. A player can therefore carry all five colours at once
   (the soul amulet's five slots) rather than needing an amulet.
2. **Soul canisters count.** `getHeartCount` sums every stored canister,
   including the soul slot, so soul contributes 2 health each. `HeartType`'s
   `SOUL(0,0)` values describe the edible soul heart/belt, not the canister.
3. **No explicit kill on a lethal removal.** Upstream calls `player.kill()`
   when the new health would be ≤ 0 (`HealthModifier.java:178-181`); the port
   clamps to 0 and lets the game's health/death path handle it. Not exercised
   by the e2e.
4. **Config stack size is static in content.** `heart_stack_size` above 10
   raises the health cap but cannot raise the item's content-declared
   `max_stack_size`.
5. **Deferred**: Curios slots, Heart Amulet/Soul Amulet and their GUI,
   heart patches, Heartpulse belts, Vitality Blade/Vigor Bow, the relic
   apple/wither bone/melted-heart chain, the coloured canister recipes (their
   ingredients are the deferred heart chain), and the starting-health base
   tweak. The empty `canister` recipe is carried.
6. **No persistence file.** The inventory is the source of truth; unlike
   solcarrot's history, nothing needs a world file. The modifier is transient
   and re-applied every tick.
7. **No client tooltip** for the canister's contribution; upstream shows the
   amulet's total on shift (`ItemHeartAmulet.java:54-58`).
8. **Mob-drop table is a documented subset.** Upstream's
   `config/bhc/drops.json` is a free-form heart-type → entity/category → chance
   map (`BHCConfig.java:8-22`, `DropHandler.java:67-112`). Slice 2 fixes the
   four shipped default entries (red `hostile`, yellow `boss`, green `dragon`,
   blue `minecraft:warden`) and exposes their chances plus a shared amount and
   an on/off switch. A pack cannot add a new mob to an entry without a code
   change. Upstream's extra wither-bone/echo-shard drops
   (`DropHandler.java:37-47`) are separate items and stay out of this slice.
9. **Category via the entity-type table.** Upstream uses `instanceof Monster`
   and the `c:bosses` entity tag. The pin's hook names only the kind, so the
   glue reads the 26.3 entity type's `category` and the pure core treats the
   wither/warden as bosses and the ender dragon separately. This matches the
   `c:bosses` set at 1.21.1 and the `Monster` hierarchy at the pin; a modded
   mob outside `monster` is treated as passive, as upstream's `instanceof
   Monster` would.
10. **Instant heart use, not a 30-tick chew.** Upstream's hearts are `EAT`
    uses of 30 ticks and heal in `finishUsingItem`
    (`ItemHeart.java:38-52,55-69`). The pin has no `use_duration` and no
    finished-use hook in its 108-hook catalog (`living_entity_use_item_finish`
    is API 90, later; see [`../_shared/hook-feasibility.md`](../_shared/hook-feasibility.md)),
    so the port heals on the `use_item` start and claims the swing. The amount,
    the creative rule and the consumption are upstream's; the eating animation
    and its 1.5 s duration are not reproduced. Because the effect is
    immediate, the solcarrot-style `player_tick` completion sampler is not
    used, and its 0-tick blind spot does not apply.

## Verification

Commands run in `ports/bhc` on 2026-11-04/05 (pin `c9d474dd…`, mixins
`d9c83d7`). `HORIZON_MIXINS=<campaign>/horizon-mixins` and
`--horizon <campaign>/horizon` are passed because the template defaults point
inside `ports/`, where the checkouts are not.

| Command | Evidence |
|---|---|
| `cargo fmt --all --check` | exit 0 (`target/bhc-slice3-fmt.log`) |
| `cargo test --locked --offline` | exit 0; `test result: ok. 39 passed; 0 failed` (`target/bhc-slice3-cargo-test.log`) |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | exit 0, **0 warnings** (`target/bhc-slice3-clippy.log`) |
| `python3 tools/horizon.py stage --horizon …` | exit 0; 2 modules, **5 injections**: `server_about_to_start`, `server_tick`, `player_tick`, `use_item`, `loot_drops` (confirmed in `target/mixins/plan.json` before cleanup; the staged tree is in `target/bhc-slice3-stage-final.log`) |
| `python3 tools/horizon.py check --horizon …` | `cargo_exit: 0`; `grep -c warning: target/horizon-check.log` → **0** (`target/bhc-slice3-check-final.log`) |
| `python3 tools/horizon.py test --horizon …` | `cargo_exit: 0`; `test result: ok. 39 passed; 0 failed; 0 ignored; 531 filtered out`; **0 warnings** (`target/bhc-slice3-test-final.log`) |
| `python3 tools/horizon.py build --horizon …` | the fast client the e2e ran is the slice-3 build at 01:12 (`target/horizon-cargo/fast/horizon-client`, 81,639,888 B); after the warning-only `Config::default` refactor (same 20/40/60/80 values) the final fast rebuild was attempted repeatedly but killed by the saturated machine (load ~42, 15 rustc; `target/bhc-slice3-build-final.log`), while the final source's `check` is **0 warnings** and `test` **39 passed** |
| `horizon-ctl` fast build | existing `target/horizon-cargo/fast/horizon-ctl` (4,812,176 B, slice-2 build; the control tool does not carry the port code) |
| `tools/e2e.sh` (default) | **exit 0, `PASS`** (`target/bhc-slice3-e2e-default.log`) |
| `MODE=control tools/e2e.sh` | **exit 0, `CONTROL PASS`** (`target/bhc-slice3-e2e-control.log`) |
| `horizon-mod build` / `check` | exit 0; delivered `content/bhc.hmod`, **13,844 B**, SHA-256 `f82f21da110a03e848e8131268075295733d007795bb990561cc6ad0ee15c2bb`, 46 files, unsigned, `"kind":"mod"` (`target/bhc-slice3-hmod-build.log`, `...-check.log`) |
| `ports/tools/clean-targets.sh` | exit 0; staged sources / incremental caches removed, **70 G free**; sources, notes, the `.hmod` and the fast binary kept (`target/bhc-slice3-clean.log`) |

The default in-game run printed (from `target/bhc-slice3-e2e-default.log`):

```
default: mob_drops=true, heart_eating=true, chance=1.0, amount=2
max health 20 (baseline)
zombie dropped 2 red hearts (configured amount 2)
cow dropped 0 hearts (no configured group)
giveable: bhc:canister
giveable: bhc:red_heart
giveable: bhc:yellow_heart
giveable: bhc:green_heart
giveable: bhc:blue_heart
giveable: bhc:red_heart_canister
giveable: bhc:yellow_heart_canister
giveable: bhc:green_heart_canister
giveable: bhc:blue_heart_canister
giveable: bhc:soul_heart_canister
max health 20 (all items cleared again)
max health 20 (64 sticks change nothing)
max health 22 (one red canister (+1 heart))
max health 24 (two red canisters (+2 hearts))
max health 26 (two red + one blue (+3 hearts))
max health 24 (blue removed (+2 hearts left))
max health 40 (12 red capped at 10 canisters (+20 hearts))
max health 20 (all canisters removed)
max health 24 (two red again)
max health 24 (bonus re-applied after respawn)
max health 24 (bonus re-applied after relaunch)
health 24 (creative baseline at full health)
health 24 (creative full health unchanged)
creative: heart count stayed 2
max health 40 (12 red capped at 10 (+20 hearts))
health 40 (canisters filled the new health)
health 16 (damaged to 16)
health 36 (red heart heals 20 (16 -> 36))
max health 40 (max health unchanged by eating)
health 24 (damaged to 24 again)
health 24 (a stick heals nothing)
health 40 (a red heart caps at 40)
health 40 (full health heals nothing)
full-health: the last red heart was consumed at 40/40
PASS: a zombie dropped 2 configured red hearts and a cow dropped none, 1 red 22, 2 red 24, +blue 26, -blue 24, 12 red capped at 40, clear 20, 24 survived respawn + relaunch, a red heart healed exactly 20 (16 -> 36) and was consumed, a stick healed nothing, creative kept its heart, and a full-health survival use still consumed one; max health never rose from eating
```

The control run (`heart_stack_size = 0`, `mob_drops = false`,
`heart_eating = false`) printed:

```
control: heart_stack_size=0, mob_drops=false, heart_eating=false, chance=1.0, amount=2
max health 20 (baseline)
control: the same zombie dropped 0 red hearts
cow dropped 0 hearts (no configured group)
giveable: bhc:canister
giveable: bhc:red_heart
giveable: bhc:yellow_heart
giveable: bhc:green_heart
giveable: bhc:blue_heart
giveable: bhc:red_heart_canister
giveable: bhc:yellow_heart_canister
giveable: bhc:green_heart_canister
giveable: bhc:blue_heart_canister
giveable: bhc:soul_heart_canister
max health 20 (all items cleared again)
max health 20 (64 sticks change nothing)
max health 20 (control: one red canister is off)
max health 20 (control: two red canisters are off)
max health 20 (control: a blue canister is off)
max health 20 (control: removing blue keeps 20)
health 20 (control: survival baseline)
health 2 (control: damaged to 2)
health 2 (control: a heart heals nothing)
control: heart count stayed 1
CONTROL PASS: canisters stayed at 20 max health with heart_stack_size=0, the zombie dropped no hearts with mob_drops=false, and a red heart neither healed nor was consumed with heart_eating=false
```

That is the differential: the identical canister sequence reaches 22/24/26
with the default cap and stays at 20 with the cap off, so an implementation
that ignored the config or the inventory would fail one of the two runs. The
same zombie drops 2 red hearts with chances at 100 % and `mob_drops = true`,
but 0 with `mob_drops = false`; the cow (passive, no configured group) drops
none in either run. A heart at 2/20 in survival does nothing with
`heart_eating = false` (health 2, count 1) but heals and is consumed with the
default. The 64-stick stack is the non-canister control and all ten content
item ids were `/give`-able before the health sequence.

Slice-2 drop table from the e2e:

| Scenario | Config | Observed |
|---|---|---|
| zombie (hostile) killed | `mob_drops=true`, red chance 1.0, amount 2 | 2 × `bhc:red_heart` |
| cow (passive, non-configured) killed | same | 0 hearts |
| zombie (hostile) killed | `mob_drops=false`, red chance 1.0, amount 2 | 0 hearts |

Client sync: the `ctl state` dump from the relaunched world reported
`"player":{"health":24.0,"max_health":24.0,...}`, i.e. the attribute modifier
reached the client copy (`take_dirty`/`lan/attributes`), not only the server
body.

Harness note: 26.3 renamed the gamerule to **`keep_inventory`**; the old
`keepInventory` spelling is rejected by the command, and without it the death
test drops the canisters (the first run failed exactly there and confirmed a
`bhc:red_heart_canister` item entity on the ground).

The in-game health table (default run) is recorded below:

| Action | Expected max health | Observed |
|---|---|---|
| baseline | 20 | 20 |
| 64 sticks (control) | 20 | 20 |
| 1 red canister | 22 | 22 |
| 2 red canisters | 24 | 24 |
| + 1 blue canister | 26 | 26 |
| − blue | 24 | 24 |
| 12 red (cap 10) | 40 | 40 |
| clear red | 20 | 20 |
| 2 red, death + respawn | 24 | 24 |
| quit + relaunch | 24 | 24 |

The slice-3 heart table (default run):

| Action | Expected | Observed |
|---|---|---|
| creative at 24/24, use a red heart | health 24, count stays 2 | 24, 2 |
| 12 red canisters (2 + 10) | max 40, health 40 | 40, 40 |
| 4 × instant damage (6 each) | health 16 | 16 |
| use a red heart (survival) | health 36, count 2 | 36, 2 |
| max health after eating | 40 (no permanent grant) | 40 |
| 2 × instant damage | health 24 | 24 |
| use a stick | health 24 | 24 |
| use a red heart at 24/40 | health 40, count 1 | 40, 1 |
| use a red heart at 40/40 | health 40, count 0 | 40, 0 |

## Skills used

- superpowers `test-driven-development` (slice 1's 16 tests written first;
  slice 2's 14 drop/config/classification tests written first; slice 3's 9
  heart-use/config tests written first and the red run failed on the missing
  types before implementation), `systematic-debugging` (slice 2's
  `next_int_bound` i32→u32 check; slice 3's `&mut` borrow on
  `PlayerMotion::attribute`, the headless guest's `lan_acting` guard, the
  dragon's win screen blocking `use-item`, and the `dead_code` warning on
  `MiniHeart::heal_amount`), `verification-before-completion` (every claim
  above comes from recorded command output).
- Beautiful Rust `rust-best-practices` (read from
  `ports/_shared/skills/BeautifulRust/skills/rust-best-practices/SKILL.md`):
  fmt/clippy `-D warnings`/tests clean, typed `Tier`/`Config`/`Carried`/
  `PlayerMode`/`HeartUse`, `Option` for tier lookup and the disabled switch,
  no bool parameters, `#[must_use]` on pure functions, no unused API.

## Re-pin `83ad384f` — 30-tick heart chew (2026-11-06)

Migrated to Horizon `83ad384f48d0742746d518737511882680aab5b7` / mixins
`e8cd0aa38cffef49cb2e088ef716baa107148c02` (`mixin_schema` 6, `game.rs`
`996432b9…`). This supersedes gap 10 above: the pin has API 90, so the
miniature hearts are a real 30-tick chew instead of an instant `use_item`.

- Hooks: `use_duration` (30 ticks when `heart_eating`), `use_animation` (Eat),
  `living_entity_use_item_finish` (heal + one-item `result` remainder unless
  creative). The old `use_item` injection/helper is removed, so the click is
  not claimed and the use starts.
- New-pin fix: the transient `max_health` modifier is re-applied from
  `player_respawned` as well as `player_tick`; at this pin the integrated
  server's respawn body can arrive without an acting-player record, and the
  per-tick hook alone left the player at 20 while the canisters remained.
- Tests: 41 passed (runtime, `game::bhc::tests`). `cargo check` and the client
  build are warning-free with the new hooks.
- E2E: default PASS (including 30-tick eat, creative keep, full-health
  consume) and `MODE=control` CONTROL PASS. Logs:
  `/tmp/opencode/bhc-e2e-default4.log`, `/tmp/opencode/bhc-e2e-control4.log`.
- The amulet (`native_item_container` + off-hand equipment) remains the next
  slice; this change does not implement it.

## Re-pin to Horizon `1d63e0a` and artifact rebuild (2026-11-09)

Mode A migration: the port pinned `83ad384f` and shipped only the `.hmod`; the
`.hnmod` half is new. The stamped hook catalog is byte-identical between the two
pins, so every injection, module and binding is unchanged; only the pin identity
moved.

| | Before | After |
|---|---|---|
| Horizon | `83ad384f48d0742746d518737511882680aab5b7` | **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`** |
| horizon-mixins | `e8cd0aa38cffef49cb2e088ef716baa107148c02` | **`5a64af938ec86b00d53d16a408aabc0038fb3e57`** |
| `game.rs` SHA-256 | `996432b9f7f9461fa69c90ff798daf6b89fe3795f379d7c6f401916633296463` | `64813cf4db0b9c3983d42908609bb633b9fbbc705f0c890396f940a6c7cef80f` |

### Ladder (through `ports/tools/build-slot.sh`, `CARGO_INCREMENTAL=0`)

| Step | Result |
|---|---|
| `cargo fmt` + `--check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** |
| `cargo test --locked --offline` | **41 passed**, 0 failed |
| `horizon.py stage` | exit 0 at `1d63e0a` |
| `horizon.py check` | `cargo_exit: 0`, **0 warnings** (64 s) |
| `horizon.py test` | **41 passed**, 1240 filtered |
| `horizon.py build` (fast) | `cargo_exit: 0` (449 s) |

### Delivered artifacts (reproducible)

| File | Size | SHA-256 | Entries |
|---|---:|---|---:|
| `content/bhc.hmod` | 13,844 B | `f82f21da110a03e848e8131268075295733d007795bb990561cc6ad0ee15c2bb` | 46 |
| `bhc.hnmod` | 17,180 B | `2ea7d63d4a9b82ce68d6071568cdab9a3c4766f9a005bdf05160348bcc460146` | 12 |

The `.hmod` is built and checked by the pin's own `cargo-horizon-mod`
(`no code: content only`); the `.hnmod` is `tools/package.py`'s deterministic
archive. A second `tools/package.sh` run reproduced both hashes.

### In-game e2e (fast build, shared `horizon-ctl`)

First run at this pin (`e2e.log`): the default leg failed at *"a red heart
caps at 40: expected health 40, got 38"*. A diagnostic run with the same
scenario plus dumps of health, the `max_health` attribute and the canister/heart
counts showed the port is correct (`health=40.0`, `max=40.0`, `red_canisters=12`
right after the eat) and the failure was the harness: the heal lands a tick
after the use, and a stray hostile hit in the End moved health by 6 between a
heal and its read. The harness now polls each health/attribute assertion for up
to ~2 s for the exact value (`condition-based waiting`, no weakened value) and
runs the health scenes at `difficulty peaceful` with hostiles cleared. The
`SELFTEST=1` counterfactual leg still fails by design (it asserts the first
canister adds nothing).

Skills: superpowers/using-superpowers,verification-before-completion,systematic-debugging; BeautifulRust/rust-best-practices

> **2026-11-10 (after this record).** A parallel campaign session converged this
> port to the registry's one-file rule by embedding the carried content in the
> `.hnmod` (`mixins/<id>/embedded_content.rs` + `mixins/<id>/data/<id>-content.hmod`).
> The `.hnmod` size/SHA-256 above is therefore from this job's split build and is
> **superseded** by that port's own later record; the pin migration, the ladder
> results and the e2e findings in this section describe the code that was tested
> here and remain the basis for the harness fixes listed.

> **2026-11-10 — embedded content; one `.hnmod` is the deliverable.**
> The built content mod (`content/bhc.hmod`) now travels inside the bundle at
> `mixins/bhc/data/bhc-content.hmod`, is compiled in with
> `horizon_client::native_data!("bhc", "bhc-content.hmod")`, written once to
> `.horizon-cache/bhc-embedded-content.hmod` and appended to the dev-mod list by
> the `embedded-content` `content_root` injection (HEAD, cancellable; the AE2
> pattern, proved first on `charginggadgets`). `tools/package.py` packs `data/`
> and refuses an embedded file that differs from `content/bhc.hmod`. One
> installed `.hnmod` therefore installs the canisters, the hearts and the code;
> the split pair is no longer a registry blocker.
>
> Artifact: `bhc.hnmod` 25,427 B, SHA-256
> `6ac84ee53bb17882b5bb15791d3580006b1a05ef3f2dc03167510e820b514ee0`,
> 14 entries (including `mixins/bhc/data/bhc-content.hmod`, 13,844 B);
> `cargo horizon-mod check bhc.hnmod --kind native-mod` passes.
> `content/bhc.hmod` stays in the port as the input of that copy, not as a
> second deliverable.
>
> In-game proof with no `--dev-mod`:
> `ports/_shared/evidence/bhc/e2e-embedded.log` first runs the control (a plain
> `.horizon-cache` file: no canister item, no heart entity, so the content
> provably comes from the bundle), then the full scenario plus a byte-compare of
> the written file against `content/bhc.hmod`; it ends `PASS` + `EXIT:0`.
> `e2e-embedded-selftest.log` ends `FAIL` + `EXIT:1`. `check-e2e-logs.py bhc
> --strict` and `upstream_ready.py bhc` are both green. Harness fix this leg
> needed: `use_held` (and the end of `launch`) closes a leftover screen — the
> End's win screen blocked `use-item` after the quit/relaunch. No assertion was
> changed.

## Reviewer repairs (2026-11-11) — PR review items and what changed


Each item below is quoted from `ports/_shared/horizon-mods/reviews/pr-*-bhc.md` (the review is the specification), with what this repair did about it.

- **Reviewer:** description truncated at 'the mod's only permanent-heart behaviour is the'
  **Repair:** Rewritten: the canister bonus is named, the inventory-scan and soul-canister limitations are stated, and the entry links the absolute upstream URL with Traverse-Joe credited.
- **Reviewer:** licence notice
  **Repair:** `LICENSE` + `LICENSE-NOTICE.md` in the `.hnmod`, `data/bhc/license/` in the carried `.hmod`.
- **Reviewer:** stale audit fixtures: `test_stage.py`/`test_template.py` assert one injection/one module, and `tools/audit/src/main.rs` rejects cancellable injections
  **Repair:** Fixed: `test_stage.py` takes the expected injection/module counts from the bundle's own manifest; `test_template.py` now checks the real pinned plan against that manifest instead of a one-function fixture game it could never stage; the audit reconstructs a cancellable HEAD's `ControlFlow::Break` guard instead of rejecting it.
- **Reviewer:** registry artifact delivery
  **Repair:** **Staging (orchestrator, not ours)**: the artifact is not in registry release/staging as `<sha256>.<ext>`; the orchestrator answers it with `horizon-mods#686` (no write access to the registry's releases). Every repackaged artifact below has a fresh sha256 for that re-push, and the author release must be re-uploaded from the port repository.

Skills: superpowers/using-superpowers,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

## Migration to the SDK format (2026-11-18, SDK `0b14f8a`)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, PR #17992) retired the `.hnmod` container
and the registry closed every `.hnmod` submission on 2026-11-18 with one reason; this port's
submission (**PR #792**) was one of them. The port is now one flat package, the shape the
**verified reference** `ports/imfast/notes.md` → `## Migration to the SDK format` records (its
12 steps and 15 traps, all followed) plus the Part-based variant `ports/repeatable_trial_vaults`
records for item parts and configs.

- **Pins:** SDK build **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (nightly 2026-11-18),
  horizon-mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, mod API **`0.2`**, package
  format **6**, launcher 0.2.78.
- **Version:** the port's own **0.2.0** (the retired bundle was 0.1.0; a resubmission is always
  a new version).
- **Class + tier unchanged:** class **content**, tier **logic** (`ports/ledger.csv`;
  `horizon/docs/modpacks.md` §Tiers). One global hook family for the health and drops, one
  `ItemPart` on four of the carried items, the config through the API's config service. No
  machine/deep feature is added.

### Files created

| Path | What it holds |
|---|---|
| `horizon-mod.toml` | the only manifest: `id = "bhc"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, the required `capabilities`, `[native]` with the exact `game`/`horizon`/`mixins` |
| `Cargo.toml` | one crate; `horizon-mod-api` from `.horizon/sdk/` and nothing else |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks` (`register`, `register_configs`, `config_loading`, `config_reloading`, `player_tick_post`, `living_entity_use_item_finish`, `loot_drops`), the `MiniHeartPart` `ItemPart`, the SDK↔runtime conversions and 2 glue tests |
| `src/runtime.rs` | the retired `mixins/bhc/runtime.rs`, **byte-identical** (SHA-256 `0757718f6adf4428dd907052d2cb142e278a18e29b99946820723550a7991b62`, `cmp` clean; identical inside the package too), 41 tests |
| `assets/bhc/…`, `data/bhc/…`, `data/c/…` | the carried content, moved out of `content/` to the package root, **byte for byte** (10 item definitions, 10 item models, 10 textures, lang, the creative tab, the empty-canister recipe and the two `c:` tag files it needs) |
| `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` |
| `LICENSE`, `LICENSE-NOTICE.md` | the licence travels **inside** the package |
| `bhc.hmod` | the delivered package, in the port root |

### Files deleted (and where the evidence went)

Everything the retired format used is under
[`ports/_shared/evidence/bhc/retired-hnmod-0.1.0/`](../../_shared/evidence/bhc/retired-hnmod-0.1.0/),
moved **before** the first build (a leftover `mixins/` path refuses the package with
`unexpected package path`):

| Deleted | Why | Kept as evidence |
|---|---|---|
| `mixins/bhc/` (`horizon-mixin.toml`, `support.rs`, `about.rs`, `player.rs`, `duration.rs`, `animation.rs`, `finish.rs`, `respawned.rs`, `server_tick.rs`, `loot.rs`, `embedded_content.rs`, `data/bhc-content.hmod`) | the plan/injection schema and per-hook helpers are not part of any SDK package; the embedded-content workaround is unnecessary because content ships in the same `.hmod` | `runtime.rs` copied to `src/` first; the rest under `retired-hnmod-0.1.0/mixins/` |
| `content/` (`horizon-mod.toml`, `assets/`, `data/`, `bhc.hmod`) | a second package; the flat package carries `assets/` and `data/` itself | `retired-hnmod-0.1.0/content/` |
| `bhc.hnmod` (28,068 B, SHA-256 `41fa01693959956ef2e165415f9708e131660ccf60a91f7ec0a96d410a0bdf30`) | unloadable on launcher ≥ 0.2.70 | `retired-hnmod-0.1.0/` |
| `compatibility.json` | the pin is `[native]` in `horizon-mod.toml` | `retired-hnmod-0.1.0/` |
| `rust-toolchain.toml` (1.95.0) | "not part of any new package"; the crate builds on the box's stable against the SDK's API | `retired-hnmod-0.1.0/` |
| `tools/horizon.py`, `tools/package.py`, `tools/package.sh`, `tools/test_stage.py`, `tools/test_template.py`, `tools/stamp_license.py`, `tools/audit/`, `tools/__pycache__` | staging, planning, the old packaging and the stale audit fixtures are gone | `retired-hnmod-0.1.0/tools/` |
| `.github/workflows/ci.yml` | the upstream template's fixture workflow: it clones `horizon-mixins`, reads `compatibility.json` and runs `tools/test_template.py`, none of which exist any more | `retired-hnmod-0.1.0/.github/` |
| `Cargo.lock` (3,704 B, SHA-256 `3aa8feb4e04b204e7e369ecd42b2721ae56b53da27b998e722e9fceaa9c7736f`) | without registry deps it is not needed, and packing one changes the artifact (trap 4 below) | not kept (recorded here) |

**Kept on purpose:** `tools/e2e.sh` (the scenario and its assertions are format-independent; it
needs a launcher-built game and is the next slice), `notes.md`, `README.md` (rewritten for the
flat layout), `VALIDATION.md`, `REPIN-AUDIT.md`, `LICENSE`, `LICENSE-NOTICE.md`.

### Helper → hook table

| Retired injection / helper | SDK (0.2) | Mapping notes |
|---|---|---|
| `server_about_to_start` (`about.rs`) reading/writing `config/bhc.conf` with `Game::native_config_dir()` | **`register_configs`** + **`config_loading`** / **`config_reloading`** | the game writes the registered default text when the file is missing, reads it and hands the text back; `Config::new("bhc", ConfigKind::Server, &default_text).file("bhc.conf")` keeps upstream's path (`BaubleyHeartCanisters.java:54-60`) and the e2e's harness |
| `player_tick` HEAD (`player.rs`) over the acting player, the host's `game.inventory`/`offhand` or the hub's guest record | **`player_tick_post(player: PlayerRef)`** | once per player on the server: `PlayerRef::inventory_slots` names the host's and every guest's slots, so the LAN special case is gone; slots 0–35 + 40 keep the retired count exactly |
| `player_respawned` (`respawned.rs`) re-applying the modifier | — (dropped) | `player_tick_post` names every player and runs after a respawn; the modifier is replaced by id every tick |
| `use_duration` HEAD (`duration.rs`) | **`ItemPart::use_duration`** on the four miniature hearts | 30 ticks while `heart_eating`; with it off no use starts at all |
| `use_animation` HEAD (`animation.rs`) | **`ItemPart::use_animation`** on the same Part | `UseAnimation::Eat` |
| `living_entity_use_item_finish` (`finish.rs`) via the acting player's body and hand | **`living_entity_use_item_finish`** (global) | `LivingRef::{health, max_health, set_health}`, `PlayerRef::game_mode`, and the consumed stack through `*event.result` (the engine puts it in the hand and syncs it) |
| `server_tick` (`server_tick.rs`) resending a guest's inventory | — (dropped) | `event.result` is the engine's own hand write, so the hub's slot-diff workaround is unnecessary |
| `loot_drops` (`loot.rs`) reading `game.res.mob_types()` for the category and `game.world.random` | **`loot_drops`** (global) | `EntityRef::classification` gives `MobCategory` (mapped to the core's `monster`/`other` string), `LevelRef::with_random` draws upstream's `nextInt(100)` from the drop's own level |
| `PlayerMotion::set_modifier` (`bhc:extra_health`, add-value, transient) | **`LivingRef::replace_attribute_modifier(…, ModifierLifetime::Transient)`** | `AttributeModifier::new(Identifier::parse("bhc:extra_health"), bonus, AttributeOperation::AddValue)`, replaced by id every tick |
| `embedded_content.rs` (`content_root` injection writing `data/bhc-content.hmod`) | — (dropped) | the flat package carries `assets/` and `data/` itself; the whole AE2 workaround is gone |

`runtime.rs` is untouched and called exactly as before: `Config`/`parse_config`/`format_config`,
`Carried`, `MiniHeart`, `class_of`, `heart_drops`, `heart_use_duration`, `next_health`,
`use_heart`, `PlayerMode`. `src/lib.rs` only converts between the SDK's types and the runtime's.

### Build and check (verbatim)

The new tool runs **directly in the port**: `build-slot.sh` still speaks the retired
`tools/horizon.py` ladder, so it is used only as the campaign's lease (it does **not** `cd` into
the port, so every command is wrapped) with `CARGO_INCREMENTAL=0` and no `--target-dir`.

```sh
ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step test \
    -- bash -c 'cd ports/bhc && CARGO_INCREMENTAL=0 cargo test --offline'
# → test result: ok. 43 passed; 0 failed   (41 runtime + 2 glue)
# the runtime alone, before any glue existed / reproducibly:
# → cargo test --offline runtime::   → 41 passed; 0 failed; 2 filtered out

ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step lint \
    -- bash -c 'cd ports/bhc && cargo fmt --check &&
        CARGO_INCREMENTAL=0 cargo clippy --all-targets --offline -- -D warnings'
# → clean (no output from fmt; clippy "Finished" with no warning)

ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step build \
    -- bash -c 'cd ports/bhc && rm -f Cargo.lock && cargo horizon-mod build'
# → SDK 0b14f8a (build 0b14f8acfd97fa2c750e42795249dcc981178ca4) in ./.horizon/sdk
# → Built ./target/horizon-mod/bhc.hmod

ports/tools/build-slot.sh run --job sdk-bhc --port ports/bhc --step check \
    -- bash -c 'cd ports/bhc && cargo horizon-mod check bhc.hmod'
# → bhc 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)
#   37466 bytes, SHA-256 b33bd4b1…, unsigned
```

`cargo horizon-mod fits bhc.hmod --build nightly` → `Baubley Heart Canisters: / 0b14f8a  made for`,
exit 0. The package's own `uses.json` (204 entries) names exactly the surfaces the glue uses:
`hook:register_configs`, `hook:config_loading`, `hook:config_reloading`, `hook:player_tick_post`,
`hook:living_entity_use_item_finish`, `hook:loot_drops`, the `ItemPart::use_duration`/
`use_animation` pair, and the handle calls (`LivingRef::replace_attribute_modifier`,
`PlayerRef::inventory_slots`, `EntityRef::classification`, `LevelRef::with_random` …).

### Package

`bhc.hmod`, **37,466 B**, SHA-256
`b33bd4b13114f268c580a7c270627dbdcb516721d990cc071e2505e8ad6d5fb1`, **53 entries**: the two
manifests, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, the ten item definitions
(`assets/bhc/items/`) and their ten `data/bhc/horizon/item/` templates, ten models, ten textures,
lang, the creative tab, the empty-canister recipe, the two `c:` tag files, `src/lib.rs`,
`src/runtime.rs` and `uses.json`. No `Cargo.lock` (trap 4). Built twice with the final sources; the two packages are byte-identical (`build-repeat-1.txt`/`build-repeat-2.txt`).

### Traps this migration cost

1. **`build-slot.sh` does not `cd` into the port.** `--port` is a label and the cache-link
   target; wrap every command as `bash -c 'cd ports/bhc && …'`.
2. **The retired paths must go before the first build** — `unexpected package path:
   mixins/bhc/runtime.rs`. Move `mixins/`, the `.hnmod`, `compatibility.json`,
   `rust-toolchain.toml`, `.github/` and the old tools into the evidence folder first.
3. **`use_duration` and `use_animation` are `ItemPart` methods now**, not global `Hooks`
   methods, and **one item takes one `ItemPart`**, so both live on the same part. The impl needs
   `#[mod_api::horizon_part]`. `ItemPart::use_animation` takes `&dyn ClientHost` (it is asked on
   the client only) while `use_duration` takes `&dyn Host`.
4. **`Cargo.lock` is packed and changes the sha256.** Any `cargo test`/`cargo clippy` writes one;
   `rm -f Cargo.lock` immediately before `cargo horizon-mod build`, and keep `/Cargo.lock` in
   `.gitignore`.
5. **`AttributeModifier` is `#[non_exhaustive]` and has no public fields for construction.**
   Use `AttributeModifier::new(Identifier, f64, AttributeOperation)`; its `id` is an
   `Identifier`, so parse the runtime's `&str` (and keep the parse fallible in glue — no
   `unwrap`).
6. **`ItemStack`/`ItemKey` are not constructible.** A drop stack is
   `host.item_key(&Identifier::parse(id)?)` then `ItemStack::new(key, count)`; the count of a
   simple stack is `ItemStack::of(key)`.
7. **The default config name is `<id>-server.toml`.** `Config::new("bhc", ConfigKind::Server, …)`
   gives it; `.file("bhc.conf")` keeps upstream's path and the existing e2e harness, and
   `config_loading` hands back `mod_id`/`file`/`text` so the mod parses its own format.
8. **`LivingEntityUseItemFinishEvent.result` is `&mut Option<ItemStack>`** — assign
   `*event.result = …`; `UseDurationEvent.duration` is a plain `i32` field the dispatcher copies
   back (`event.duration = ticks`).
9. **`csv`-style `Hooks` signature drift is silent no more:** every method name was taken from
   the generated trait (`cargo horizon-mod hooks <text>`, `surface.json`), never guessed.
10. **`src/runtime.rs` must be `pub mod runtime;`** — a private module turns every `pub` item in
    the carried file into a `dead_code` warning, and the file itself must stay byte-identical
    (do not add an `allow` to it).
11. **A `horizon-part`-only mod still declares `capabilities = ["registries"]`**: the manifest
    value is a permission and is not negotiable, even though the check report's own
    `capabilities` is `[]` for a native package.
12. **No `rust-version` in `Cargo.toml`.** The scaffold declares none; the SDK's toolchain
    (1.99.1) is the launcher's, and the box's stable compiles the crate.

### Gaps after the migration

1. **No in-game run at `0b14f8a`.** `horizon-launcher` is not installed on this box and the
   published shared binaries stop at `1d63e0a5`/`41f822b6`, so no game carrying this mod can be
   built here. `tools/e2e.sh` keeps the scenario and its assertions (they are format-independent)
   and the next slice is "build the game with the launcher, point `GAME=` at it, run
   `tools/e2e.sh` (default + `MODE=control` + `SELFTEST=1`) and record the transcript". Until
   then the migration's own claims are the carried rules (41 tests), the type-checked glue, and
   the registry's `check`/`fits`.
2. **No log service in the SDK.** The retired bundle's `[bhc] …` lines (`log::info!`/`log::warn!`)
   are gone: `log` is not re-exported by `horizon-mod-api` and a mod cannot add a crate. The port
   is silent; the e2e's `grep`-based lines must become state assertions (they already are).
3. **The modifier is written server-side only.** `replace_attribute_modifier` is a server
   operation; the client's copy comes from the server's attribute sync (`/attribute`'s own path).
   The retired bundle also wrote the local body, which was redundant and could disagree.
4. **The config now reloads.** `config_reloading` applies an edited `bhc.conf` within a second
   (the API's own poll); the retired bundle read the file once at server start. This is an
   addition, not a change of defaults.
5. **The e2e's `EMBEDDED=1`/embedded-cache legs are retired** (there is no embedded content half
   to write): a launcher-built game loads the one `.hmod` and the content ids come from it. The
   `--dev-mod` control leg becomes "the same game without the mod", which is the honest
   differential at this format.
6. Upstream gaps unchanged: the Curios heart amulet and its GUI, heart patches, belts, weapons,
   mob drops beyond the four shipped defaults, the melted-heart recipe chain, the starting-health
   tweak, and the inventory-scan stand-in for the amulet's slots (see `## Deviations and gaps`).

### Changelog for the resubmission

> Supersedes #792, closed because Horizon retired `.hnmod` (#17992). Changes: the flat SDK package
> with `api = "0.2"` and `[native]` pinned to `0b14f8a`/`e59da90`; the hook bodies rewritten as
> `impl mod_api::Hooks` methods against typed hosts and events (`register_configs` +
> `config_loading` for `config/bhc.conf`, `player_tick_post` per player, `loot_drops` with
> `EntityRef::classification`, `living_entity_use_item_finish` handing the consumed stack back
> through `event.result`); the 30-tick chew moved from global `use_duration`/`use_animation`
> injections to one `ItemPart` on the four miniature hearts; the max-health modifier applied
> through `LivingRef::replace_attribute_modifier` instead of a pin-internal body write; the
> carried items, textures, lang, recipes and tags ship inside the package (`assets/`, `data/`), so
> the embedded-content injection and its cache file are gone; `server_tick`'s guest inventory
> resend is gone with them; `src/runtime.rs` and its 41 tests are unchanged. Version 0.2.0.

Skills: superpowers/using-superpowers,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

### Verification (migration)

| Check | Command | Result |
|---|---|---|
| Runtime carried | `cmp src/runtime.rs ../_shared/evidence/bhc/retired-hnmod-0.1.0/mixins/bhc/runtime.rs` | identical, SHA-256 `0757718f…1b62` (`evidence/bhc/sdk-0b14f8a/runtime-carry.log`); the copy inside the package has the same hash |
| Runtime tests alone | `cargo test --offline runtime::` | **41 passed**, 0 failed, 2 filtered (`runtime-test-only.log`) |
| All tests | `cargo test --offline` | **43 passed**, 0 failed (`unit.log`) |
| Formatting | `cargo fmt --check` | clean, exit 0 (`fmt.log`, empty) |
| Lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings (`clippy.log`) |
| Package | `cargo horizon-mod build` | `target/horizon-mod/bhc.hmod`, exit 0; 37,466 B, sha256 `b33bd4b1…` (`build.log`, `sha256.txt`) |
| Registry check | `cargo horizon-mod check bhc.hmod` | `bhc 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)`, exit 0 (`check.log`, `check-json.log`) |
| Fits the SDK build | `cargo horizon-mod fits bhc.hmod --build nightly` | `0b14f8a  made for`, exit 0 (`fits.log`) |
| Surfaces used | `unzip -p bhc.hmod uses.json` | 204 entries naming the 7 hooks and the two Part methods (`uses.json`) |
| Package entries | `unzip -l bhc.hmod` | 53 entries, no `Cargo.lock` (`package-entries.log`) |

Evidence: [`ports/_shared/evidence/bhc/sdk-0b14f8a/`](../../_shared/evidence/bhc/sdk-0b14f8a/).
