# Baubley Heart Canisters — re-pin audit against Horizon `83ad384f`

Read-only audit, 2026-11-05. **No port source, content, manifest or tool was
changed; no build, test or e2e was run.**

| | Old | New |
|---|---|---|
| Horizon | `c9d474dd9116e84c243f8992d75c11314df56f06` (108 hooks) | **`83ad384f48d0742746d518737511882680aab5b7`** (427 hooks, 159 helpers, `NATIVE_HOOKS_API = 409`) |
| horizon-mixins | `d9c83d7096b7ea5b83678da2efdfd6161f74997e` | **`e8cd0aa38cffef49cb2e088ef716baa107148c02`** |
| plan schema | 6 | 6 (unchanged) |

Sources: `ports/_shared/pins/horizon-83ad384f/native-hooks.json`,
`crates/horizon-client/src/**`, `crates/horizon-world/src/**`,
`docs/native-mods.md`, `docs/mods.md`, `crates/horizon-mods/src/content.rs`;
this port's `notes.md` and `../_shared/upstream/bhc/FINDINGS.md`.

Classification: **usable** = a writable/cancellable hook or a helper exists;
**hook-only** = only a listen hook; **absent** = no hook/helper.

Method: each deferral in `notes.md`/`FINDINGS.md` was mapped to
`native-hooks.json` and the `horizon-client`/`horizon-world` helpers;
content-only items were checked in `horizon-mods`/`docs/mods.md`. The brief
states main `696d0216` has the same hook set; this audit uses the pinned
export.

## Re-pin safety (existing injections)

All five existing hooks still exist at the new pin. `server_tick` gained a
`context` parameter, but the TOML binds only named parameters, so
`{ game, phase }` still resolves. The wrapper must be migrated first, or
`test` runs zero tests.

| Existing hook | New-pin signature | Verdict |
|---|---|---|
| `server_about_to_start` | `native_server_about_to_start(&mut self, dedicated: bool)` — `native_hooks.rs:2404` | unchanged; `dedicated` unbound |
| `player_tick` | `native_hooks.rs:2547` | unchanged |
| `use_item` | `native_hooks.rs:6523` | unchanged |
| `server_tick` | `native_hooks.rs:2456` (`context`, `phase`) | extra `context`; bindings still resolve |
| `loot_drops` | `native_hooks.rs:7458` | unchanged |

Re-pin steps from [`../_shared/repin-83ad384f.md`](../_shared/repin-83ad384f.md):
new horizon/mixins revisions and `game.rs` SHA-256
`996432b9f7f9461fa69c90ff798daf6b89fe3795f379d7c6f401916633296463`;
manifest `[compatibility] horizon_revision` must match; `tools/horizon.py:121-122`
must use `--lib` (the old `-p horizon-client --bin horizon-client` runs 0
tests); `horizon-ctl` is now `-p horizon-client-lib --bin horizon-ctl`; one
online `cargo fetch --locked` for `zune-jpeg`. Not executed here.

## Deferred features

| ID | Deferred feature (notes) | Best new-pin API (symbol, file:line) | Class | Verdict | Next step |
|---|---|---|---|---|---|
| B1 | Curios slots / amulet worn outside the inventory (`notes.md:232-236`) | `native_living_equipment` `native_living_refs.rs:73`; `native_living_set_equipment` `native_living_refs.rs:122`; `native_on_equip` `native_hooks.rs:7623`; API 382/39 `docs/native-mods.md:1648,326` | usable (vanilla slots only); **custom slot registry absent** | **partial** — a player has main hand, off hand, head, chest, legs, feet only (`native_living_refs.rs:113-119`); no Curios-like slot | Put the amulet in the off hand (or keep the inventory scan) and give it native item-container slots. Record the custom-slot form as an API request. |
| B2 | Heart Amulet / Soul Heart Amulet items (`notes.md:247-251`, `FINDINGS.md:40-44`) | `native_item_container` `native_hooks.rs:3109`; `native_item_container_slots` `native_item_container.rs:166`; `insert` `:181`; `extract` `:194`; API 370 `docs/native-mods.md:1622` | usable | **realizable now** — 4 (plain) and 5 (soul) slots, contents in `minecraft:container`, saved and sent with the stack | Add the two amulet items to `content/` with `native_item_container(item, 4/5)` and a `use_item` equip; read canisters from `item_container_slots` instead of scanning the inventory. |
| B3 | Amulet GUI (`notes.md:247-251`) | `native_open_menu` `native_menus.rs:124`; `native_set_menu_items` `native_menus.rs:143`; `native_menu_clicked` `native_hooks.rs:6391`; `native_menu_closed` `native_hooks.rs:6403`; API 10/19 `docs/native-mods.md:171-208` | usable | **realizable now**, but manual: `open_menu` builds a vanilla grid from items; the port syncs clicks to the amulet’s container itself | Open a 1-row menu from the amulet’s 4/5 slots; on `menu_clicked` route to `item_container_insert/extract`; on close write the container back. E2E: put a canister in, close, reopen, health updates. |
| B4 | Heart patches (`notes.md:247-251`) | `native_use_duration` `native_hooks.rs:6537`; `native_use_animation` `:6547`; `native_can_eat` `:6555`; `native_living_entity_use_item_start` `:6571`; `finish` `:6597`; `tick` `:6635`; `stop` `:6679`; API 90 `docs/native-mods.md:939` | usable | **realizable now** | Same consumable path as B14; a patch is a heart with a config heal/cooldown. Add to the existing `MiniHeart` table rather than a second hook. |
| B5 | Heartpulse belts (`notes.md:247-251`) | attribute modifiers through `PlayerMotion::set_modifier` (already used) + `native_inventory_tick` `native_hooks.rs:7614` / `native_carried_stack` `:8450`; API 39 `docs/native-mods.md:326` | usable | **realizable now** | Content items + a belt modifier table in `runtime.rs`; refresh from `inventory_tick`/`player_tick`. No new API. |
| B6 | Vitality Blade (`notes.md:247-251`) | `native_living_incoming_damage_v2` `native_hooks.rs:7736` (host-aware player→mob melee; `amount: &mut f32`); `native_hurt_enemy` `:7601`; `native_attack_entity` `:6855`; API 384/39/22 `docs/native-mods.md:1652,326,218` | usable | **realizable now** for mob hits; player victims still only v1 (static) | Scale damage in v2 from the attacker/victim state; for PvP use a cached-health `player_tick` map in v1 (same pattern as artifacts). Content item + a runtime damage table. |
| B7 | Vigor Bow (`notes.md:247-251`) | `native_living_get_projectile` `native_hooks.rs:5497` (replace ammunition); `native_entity_shoot_bow` `:5456`; `native_arrow_loose` `:6705` (charge/cancel); `native_projectile_launch` `:5444`; `native_living_incoming_damage` `:7709` (`damage_type`, `direct`, `amount`); API 376/384 | usable | **realizable now** with a cached shooter-health map for the static hit hook; v2 does not cover projectiles (`docs/native-mods.md:1652`) | Tag the arrow at launch (`living_get_projectile`/`arrow_loose`), cache the shooter’s health each tick, scale `amount` in v1 when `damage_type` is an arrow and `causing` is that player. E2E: full-health vs half-health arrow damage. |
| B8 | Relic apple / wither bone / melted-heart chain (`notes.md:247-251`, `FINDINGS.md:78-84`) | `native_loot_drops` `native_hooks.rs:7458` for wither bone/echo shard; content items + shaped recipes; API 75/12+ | usable | **realizable now** (content + one loot hook) | Carry the chain’s items/recipes; add the wither-bone/echo-shard entries to the existing `loot_drops` helper. No new API. |
| B9 | Coloured canister recipes (`notes.md:247-251`) | content recipes; the ingredients are B8’s chain | usable | **realizable now** (content-only) | Carry the four/two recipes once B8’s items exist; check the 26.3 recipe schema in `content/`. |
| B10 | Starting-health base tweak (`notes.md:247-251`) | `native_player_logged_in` `native_hooks.rs:6949`; `native_player_load_from_file` `:6997`; `PlayerMotion::set_modifier`; API 21/31 | usable | **realizable now** | Apply a `bhc:starting_health` `max_health` modifier once at login/load when enabled; use `add_value` with the configured base delta, not a base-value write (no base-value helper). |
| B11 | No persistence file (`notes.md:252-254`) | `native_attachments` `native_hooks.rs:8406` + `AttachmentType` (`docs/native-mods.md:3384-3444`); or `native_world_dir` `native_server.rs:495` / `native_config_dir` `native_hooks.rs:9216`; API 317 | usable | **realizable now** | Use a player attachment with a codec for per-player cooldowns/undo, or `world_dir` for a world file. The inventory remains authoritative for canisters. |
| B12 | No client tooltip (`notes.md:255-256`) | `native_tooltip` `native_hooks.rs:6413` (`collect: lines`); API 12 `docs/native-mods.md:175` | usable | **realizable now** | Append the canister total and the amulet’s stored colours; pure `runtime.rs` line builder, thin client helper. |
| B13 | Mob-drop table is a fixed subset (`notes.md:257-264`) | `native_register_configs` `native_hooks.rs:2424` + `native_config_loading` `:2432`; or `native_add_server_reload_listeners` `:2343` + `native_server_reload_listener_apply` `:2357`; API 397/400 `docs/native-mods.md:1678,1684` | usable | **realizable now** | Register a `Server` config (`config/bhc-drops.json` text) and parse upstream’s map; the existing `loot_drops` helper consults it. E2E: add a modded mob entry, restart, drop appears. |
| B14 | Instant heart use, no 30-tick chew (`notes.md:272-281`) | `native_use_duration` `native_hooks.rs:6537`; `native_use_animation` `:6547`; `native_can_eat` `:6555`; `native_living_entity_use_item_start` `:6571`; `native_living_entity_use_item_finish` `:6597` (mutable `result: Option<ItemStack>`); `tick` `:6635`; `stop` `:6679`; API 90 `docs/native-mods.md:939` | usable | **realizable now** — this is the single biggest correction to the slice-3 notes | Stop claiming `use_item` for hearts; add `use_duration` (30), `use_animation` (Eat), and `living_entity_use_item_finish` (heal + one-item remainder unless creative). Keep an instant-use config flag for servers that want it. E2E: hold use 1.5 s, health rises at the end; letting go early heals nothing. |
| B15 | No explicit kill on lethal removal (`notes.md:240-243`) | `PlayerMotion::set_health` (already used) + `native_server_tick` `native_hooks.rs:2456` + `native_run_command` `native_server.rs:479` (from a non-player hook) ; `native_living_death` `:6045` is listen/cancel, not force | usable | **realizable now** | When the new max would be ≤ 0, queue `kill <name>` from `server_tick` (or set health 0 and let the death path run). Do not call `run_command` inside the player hook (`native_server.rs:476-478`). E2E: remove all canisters at 2 health → death. |
| B16 | Config stack size is static in content (`notes.md:244-246`) | `native_inventory_tick` `native_hooks.rs:7614` + `native_carried_stack` `:8450` + `native_set_component` `native_components.rs:49` (`minecraft:max_stack_size`); API 39/65 `docs/native-mods.md:326,825` | usable | **realizable now**, but probe first: the stack component must be honoured by the container/stack code | Rewrite the canister stack’s `minecraft:max_stack_size` from the config in `inventory_tick`; if the engine reads only the item default, keep the documented static limit. Do not change content yet. |

## Summary

- **Deferred features audited: 16** — the notes’ gaps 1, 3–8 and 10, with the
  amulet and its GUI split (B2/B3) and the “other content” list expanded
  (B5–B10). B14 is the 30-tick-chew gap; B1 is the Curios/amulet form.
- **Realizable now in full: 15** — B2–B16. Every one has a concrete
  hook/helper path.
- **Newly opened by `83ad384f`: 7** — B2/B3 (item containers + menus/GUI),
  B4/B14 (use-over-time, including the 30-tick eat), B11 (attachments), B13
  (registered/reloadable config), B16 (component writes). B1 is newly
  *partial*: equipment slots now exist, but only vanilla ones.
- **Partial: 1** — B1 (no Curios-equivalent custom slot; off-hand/container
  approximation).
- **Still fully blocked: 0.** The only missing form is a custom equipment
  slot, and it has a workable approximation.
- **Highest-value next step:** implement **B1+B2+B3 as one “amulet” slice** —
  the amulet as an off-hand item with 4/5 native item-container slots, a menu
  to fill it, and canister health read from the container. That removes the
  mod’s central deviation (the inventory scan standing in for the amulet).
  **B14 (30-tick chew)** is the cheapest immediate win and should be the
  first commit: it edits only the existing heart-use path and removes the
  most visible remaining shortcut.

## Gotchas found while reading

- `native_living_set_equipment` returns false for a player’s body/saddle slot
  (`native_living_refs.rs:113-119`); use `OffHand` (or an armour slot) for the
  amulet, not a fictional curio slot.
- `take_items`/`count_items` only see plain stacks (`damage == 0`, no
  components; `native_server.rs:503-505`). A canister/amulet with a container
  or damage component must be moved with `native_carried_stack` or the
  inventory helpers, not by id.
- `native_use_item` does not name the hand. Track the selected hotbar slot
  from `inventory_tick`’s `selected` flag (or `native_player_item_held`) and
  compare with `native_held()` before clearing/equipping, or the amulet can be
  removed from the off hand it was just equipped into.
- `native_use_duration` applies to a mod item with no vanilla use; the heart
  items need no `food`/`consumable` component and should **not** claim
  `use_item` once the chew is implemented.
- `native_living_entity_use_item_finish` receives a mutable replacement
  result; that is the clean place for the one-item remainder (and it is
  server-side, so the creative rule is the port’s).
- `native_run_command` is refused inside a player hook
  (`native_server.rs:476-478`); queue the lethal `/kill` from `server_tick`.
- `native_attachments` saves per `Holder::Player(uuid)`, so a cooldown map
  survives relog without a new world file.
