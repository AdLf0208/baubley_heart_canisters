// Game-side glue for Baubley Heart Canisters. Injected into `crate::game` as
// `bhc_state`; it needs the game, so it is not part of the standalone library
// (only `runtime.rs` is).
//
// Upstream grants the max-health bonus only while a Heart Amulet is equipped
// in a Curios slot (`ItemHeartAmulet.java:61-81`). Slice 1 defers Curios and
// the amulet, so the player's carried main inventory stands in for the
// amulet's canister slots: the glue scans it every player tick, caps each
// colour at `heart_stack_size`, and applies the upstream modifier. Because the
// inventory is the source of truth, the value survives respawns and dimension
// changes by simply being re-applied on the next tick.

use std::collections::BTreeSet;
use std::fs;
use std::sync::Mutex;

use horizon_client::items::ItemStack;
use horizon_core::random::RandomSource;
use horizon_world::consumable::UseAnimation;

use crate::first_person::Hand;
use crate::game::bhc::{
    CONFIG_FILE, Carried, Config, HEALTH_MODIFIER_ID, MiniHeart, PlayerMode, format_config,
    heart_drops, heart_use_duration, next_health, parse_config, use_heart,
};
use crate::game::native_hooks::NativeLootDrops;

#[derive(Default)]
struct Core {
    config: Config,
    /// Players whose inventory needs an explicit resend after an in-use
    /// decrement (`hand_stack_replaced` only covers the host's own client).
    sync: BTreeSet<u128>,
}

static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn core() -> std::sync::MutexGuard<'static, Option<Core>> {
    CORE.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// The loaded server settings, or upstream's defaults before the server
/// reaches `server_about_to_start`.
fn config() -> Config {
    let guard = core();
    guard
        .as_ref()
        .map_or_else(Config::default, |core| core.config.clone())
}

/// Before the server starts: read `config/bhc.conf`, writing upstream's
/// defaults when it is absent (`ConfigHandler.java:27-29`).
#[allow(private_interfaces)]
pub fn on_server_about_to_start(_game: &mut crate::game::Game<'_>) {
    let mut config = Config::default();
    match crate::game::Game::native_config_dir() {
        Ok(dir) => {
            let path = dir.join(CONFIG_FILE);
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    let text = format_config(&config);
                    if fs::create_dir_all(&dir)
                        .and_then(|()| fs::write(&path, &text))
                        .is_err()
                    {
                        log::warn!("[bhc] could not write {}", path.display());
                    }
                    text
                }
            };
            config = parse_config(&text);
            log::info!(
                "[bhc] config {} (heart_stack_size={}, heart_eating={})",
                path.display(),
                config.heart_stack_size,
                config.heart_eating
            );
        }
        Err(error) => log::warn!("[bhc] no config dir: {error}"),
    }
    let mut guard = core();
    let core = guard.get_or_insert_with(Core::default);
    core.config = config;
}

/// End of a player's server tick: count the canisters they carry and keep the
/// max-health modifier in step. Re-applying every tick covers respawns and
/// dimension changes (the pin's modifier is transient).
#[allow(private_interfaces)]
pub fn on_player_tick(game: &mut crate::game::Game<'_>) {
    // The acting player is normally swapped in, but on the integrated server
    // after a respawn the new body can be present without an acting-player
    // record; fall back to the local body so the transient modifier is
    // re-applied (a new-pin behaviour; canisters are still in the inventory).
    let uuid = game.native_acting_player().map(|player| player.uuid);
    let carried = scan_carried(game, uuid);
    apply_health(game, &carried, &config());
}

/// A player respawned (API 21): the new body replaces the old one, so the
/// transient max-health modifier must be re-applied here as well as in the
/// per-tick hook. The respawn hook runs with the player's state swapped in.
#[allow(private_interfaces)]
pub fn on_player_respawned(game: &mut crate::game::Game<'_>, _player: u128, _name: &str) {
    on_player_tick(game);
}

/// `Item.getUseDuration` (API 90): a miniature heart is eaten over 30 ticks
/// when `heart_eating` is on; with it off no use starts at all, so the heart
/// is inert without needing a `use_item` veto.
pub fn on_use_duration(stack: &ItemStack, duration: &mut i32) {
    let Some(heart) = MiniHeart::from_item(&stack.item) else {
        return;
    };
    if let Some(ticks) = heart_use_duration(heart, &config()) {
        *duration = ticks;
    }
}

/// `Item.getUseAnimation` (API 90): the first-person eat pose for a heart.
pub fn on_use_animation(stack: &ItemStack, animation: &mut UseAnimation) {
    if MiniHeart::from_item(&stack.item).is_some() {
        *animation = UseAnimation::Eat;
    }
}

/// `LivingEntityUseItemEvent.Finish` (API 90): heal the acting player by
/// upstream's `HeartType.healAmount` and replace the hand with one fewer heart
/// unless they are creative (`ItemHeart.java:55-68`). This is the server-side
/// completion of the 30-tick chew; the engine owns the animation and the
/// held-stack write, so the old manual `use_item` claim and inventory resend
/// are gone.
#[allow(private_interfaces)]
pub fn on_use_item_finish(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    entity: Option<i32>,
    _hand: Hand,
    result: &mut Option<ItemStack>,
) {
    // Only a player has the health/mode this port reads; a mob's use has no
    // page in upstream's item.
    if entity.is_some() {
        return;
    }
    let Some(heart) = MiniHeart::from_item(&stack.item) else {
        return;
    };
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let uuid = player.uuid;
    let (health, max) = {
        let Some(body) = game.player.as_mut() else {
            return;
        };
        let max = body.attribute("minecraft:max_health").unwrap_or(20.0) as f32;
        (body.health.health(), max)
    };
    let Some(outcome) = use_heart(
        heart,
        health,
        max,
        PlayerMode::of(player.game_mode),
        &config(),
    ) else {
        return;
    };
    if (outcome.health - health).abs() > f32::EPSILON
        && let Some(body) = game.player.as_mut()
        && let Err(error) = body.set_health(outcome.health, false)
    {
        log::warn!("[bhc] set health after {}: {error}", heart.item_id());
    }
    if outcome.consume {
        let current = (*result).clone().unwrap_or_else(|| stack.clone());
        *result = if current.count > 1 {
            Some(ItemStack {
                count: current.count - 1,
                ..current
            })
        } else {
            None
        };
        if game.lan_acting.is_some() {
            // The hub's slot diff misses a use-completion decrement during the
            // guest's turn; resend at tick end (akashictome's verified fix).
            let mut guard = core();
            let core = guard.get_or_insert_with(Core::default);
            core.sync.insert(uuid);
        }
    }
}

/// At the end of a running server tick, resend the inventory of every guest
/// whose heart use decremented a stack.
#[allow(private_interfaces)]
pub fn on_server_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::ServerTickPhase,
) {
    let crate::game::native_lifecycle::ServerTickPhase::ServerTickEnd { world_running } = phase
    else {
        return;
    };
    if !world_running {
        return;
    }
    let sync = {
        let mut guard = core();
        let Some(core) = guard.as_mut() else {
            return;
        };
        std::mem::take(&mut core.sync)
    };
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    for uuid in sync {
        if let Some(index) = host
            .hub
            .guests
            .iter()
            .position(|guest| guest.placed && !guest.dead && guest.uuid == uuid)
        {
            host.hub.send_inventory(index);
        }
    }
}

/// A mob's drops are being rolled (`loot_drops`, API 75). Upstream adds the
/// `drops.json` miniature hearts in `LivingDropsEvent`
/// (`DropHandler.java:32-52,58-119`); this port does the same through the
/// hook. Only a mob death's loot is touched: a block break names no entity, a
/// player's use sets `used`, and a leashed mob's lead drop has no table.
#[allow(private_interfaces)]
pub fn on_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    let Some((_, kind)) = context.entity else {
        return;
    };
    if context.used || context.table.is_empty() {
        return;
    }
    let config = config();
    if !config.mob_drops || config.heart_drop_amount == 0 {
        return;
    }
    let category = game
        .res
        .mob_types()
        .get(kind)
        .map_or("", |mob| mob.category.as_str())
        .to_owned();
    let rolled = {
        let random = &mut game.world.random;
        heart_drops(kind, &category, &config, || {
            u32::try_from(random.next_int_bound(100).unwrap_or(0)).unwrap_or(0)
        })
    };
    drops.extend(
        rolled
            .into_iter()
            .map(|drop| plain_stack(drop.heart.item_id(), drop.count)),
    );
}

/// A default-component stack of `count` `item`, the only shape these drops
/// need (upstream constructs `new ItemStack(item)`).
fn plain_stack(item: &str, count: u32) -> ItemStack {
    ItemStack {
        item: item.into(),
        count,
        damage: 0,
        extra: None,
    }
}

/// The canisters in one player's main inventory and off hand. A LAN guest's
/// inventory lives in the hub's guest record, not in the host's slots.
fn scan_carried(game: &crate::game::Game<'_>, uuid: Option<u128>) -> Carried {
    if let Some(host) = game.lan.as_ref()
        && let Some(uuid) = uuid
        && let Some(guest) = host
            .hub
            .guests
            .iter()
            .find(|guest| guest.placed && !guest.dead && guest.uuid == uuid)
    {
        let stacks = guest
            .inventory
            .iter()
            .flatten()
            .map(|stack| (stack.item.as_ref(), stack.count));
        let mut carried = Carried::from_stacks(stacks);
        if let Some(stack) = guest.offhand.as_ref() {
            carried.add_stack(stack.item.as_ref(), stack.count);
        }
        return carried;
    }
    let stacks = game
        .inventory
        .iter()
        .flatten()
        .map(|stack| (stack.item.as_ref(), stack.count));
    let mut carried = Carried::from_stacks(stacks);
    if let Some(stack) = game.offhand.as_ref() {
        carried.add_stack(stack.item.as_ref(), stack.count);
    }
    carried
}

/// Apply (or update) this mod's max-health modifier and keep the current
/// health at the same distance below max, as `updatePlayerHealth` does
/// (`HealthModifier.java:149-182`). The pin's `set_modifier` adds a transient
/// modifier, which the next tick re-applies if a respawn dropped it.
fn apply_health(game: &mut crate::game::Game<'_>, carried: &Carried, config: &Config) {
    let desired = f64::from(carried.health_bonus(config));
    let Some(player) = game.player.as_mut() else {
        return;
    };
    // Re-set the modifier every tick: at this pin a respawn can keep the
    // modifier record while the new body's computed max health does not include
    // it, so comparing the record alone would leave the player at 20. Setting
    // it by id is idempotent and also covers respawns/dimension changes.
    let old_max = player.attribute("minecraft:max_health").unwrap_or(20.0) as f32;
    let old_health = player.health.health();
    if let Err(error) = player.set_modifier(
        "minecraft:max_health",
        HEALTH_MODIFIER_ID,
        desired,
        "add_value",
        true,
    ) {
        log::warn!("[bhc] max health modifier: {error}");
        return;
    }
    let new_max = player
        .attribute("minecraft:max_health")
        .unwrap_or(f64::from(old_max)) as f32;
    let new_health = next_health(old_health, old_max, new_max);
    if let Err(error) = player.set_health(new_health, false) {
        log::warn!("[bhc] set health {new_health}: {error}");
    }
}
