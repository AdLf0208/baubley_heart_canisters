//! Baubley Heart Canisters — the one `impl Hooks` at the crate root.
//!
//! A carried canister stack raises the carrier's maximum health by two points
//! (`HealthModifier.java:161`), each colour capped by `heart_stack_size`; the miniature hearts
//! are eaten over 30 ticks and heal their tier's amount (`ItemHeart.java:38-69`); mobs drop
//! them through `loot_drops` with upstream's `drops.json` defaults.
//!
//! The same shape the retired `.hnmod` had, one hook per moment:
//!
//! | Retired injection | SDK |
//! |---|---|
//! | `server_about_to_start` reading `config/bhc.conf` by hand | `register_configs` + `config_loading`/`config_reloading` (the game writes and reads the file) |
//! | `player_tick` (acting player only) | `player_tick_post`, once per player (host and guests alike) |
//! | `use_duration` / `use_animation` global hooks | an [`ItemPart`] on the four miniature hearts |
//! | `living_entity_use_item_finish` writing health + the hand | the same hook, with `event.result` carrying the consumed stack |
//! | `loot_drops` reading the mob table for a category | the same hook, with `EntityRef::classification` |
//! | `player_respawned` re-applying the modifier | dropped: `player_tick_post` names every player and runs after a respawn |
//! | `server_tick` resending a guest's inventory | dropped: `event.result` is what the engine hands back |
//!
//! The rules themselves live in [`runtime`], byte-identical to the retired bundle's
//! `runtime.rs` and tested by `cargo test` without the game.
use std::sync::{Arc, Mutex, PoisonError};

use mod_api::{
    self, AttributeModifier, AttributeOperation, ClientHost, Config as ModConfig, ConfigFile,
    ConfigKind, ConfigLoadingEvent, ConfigReloadingEvent, GameMode, Hooks, Host, HostSide,
    Identifier, ItemKey, ItemPart, ItemStack, LivingEntityUseItemFinishEvent, LootDropsEvent,
    MobCategory, ModifierLifetime, PlayerRef, PlayerTickPostEvent, RandomSource,
    RegisterConfigsEvent, Registrar, RegistrationContext, ServerHost, UseAnimation,
    UseAnimationEvent, UseDurationEvent,
};

pub mod runtime;

use runtime::{
    CONFIG_FILE, Carried, HEALTH_MODIFIER_ID, MiniHeart, PlayerMode, ServerConfig, format_config,
    heart_drops, heart_use_duration, next_health, parse_config, use_heart,
};

/// The attribute the canisters raise (`HealthModifier.java:19,171`).
const MAX_HEALTH: &str = "minecraft:max_health";

/// The inventory slots the retired bundle counted: the main inventory (0–35) and the off hand
/// (40), not the armour or body slots a canister can never sit in.
const COUNTED_SLOTS: std::ops::RangeInclusive<u32> = 0..=35;
const OFF_HAND_SLOT: u32 = 40;

/// The session state the hooks and the item Parts share: the server settings.
type Shared = Arc<Mutex<State>>;

#[derive(Default)]
struct State {
    /// Upstream's defaults until the game hands the file back at `config_loading`.
    config: ServerConfig,
}

impl State {
    fn config(&self) -> ServerConfig {
        self.config.clone()
    }
}

#[derive(Default)]
struct Mod {
    state: Shared,
}

impl Mod {
    fn config(&self) -> ServerConfig {
        lock(&self.state).config()
    }

    /// `ModConfigEvent.Loading`/`Reloading`: keep the text this mod's server file holds.
    fn read_config(&self, file: &ConfigFile) {
        if file.mod_id != "bhc" || file.file != CONFIG_FILE {
            return;
        }
        lock(&self.state).config = parse_config(&file.text);
    }
}

/// The state lock is only ever held for a field read or write; poison means another hook
/// panicked while holding it, and continuing with a half-written config would be worse.
fn lock(state: &Shared) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Attach the 30-tick eat to the four miniature hearts: the use time and the eat pose.
    fn register(&self, r: &mut dyn Registrar, _context: &RegistrationContext) {
        for heart in MiniHeart::ALL {
            let item = r.item_key(heart.item_id());
            r.item_part(
                item,
                MiniHeartPart {
                    heart,
                    state: Arc::clone(&self.state),
                },
            );
        }
    }

    /// The server settings live in `config/bhc.conf`, upstream's path
    /// (`BaubleyHeartCanisters.java:54-60`): the game writes the defaults when the file is
    /// missing and hands the text back at `config_loading`.
    fn register_configs(&self, _host: HostSide, event: &mut RegisterConfigsEvent<'_>) {
        let config = ModConfig::new(
            "bhc",
            ConfigKind::Server,
            &format_config(&ServerConfig::default()),
        )
        .file(CONFIG_FILE);
        event.configs.push(config);
    }

    fn config_loading(&self, _host: HostSide, event: &mut ConfigLoadingEvent<'_>) {
        self.read_config(event.config);
    }

    fn config_reloading(&self, _host: HostSide, event: &mut ConfigReloadingEvent<'_>) {
        self.read_config(event.config);
    }

    /// After each player's tick: count the canisters they carry and keep the max-health
    /// modifier in step. It runs for every player, so the LAN guest the retired bundle had to
    /// find in the hub is covered, and re-applying every tick covers respawns and dimension
    /// changes because the modifier is transient.
    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Some(carried) = carried_canisters(&*host, event.player) else {
            return;
        };
        apply_health(host, event.player, &carried, &self.config());
    }

    /// `LivingEntityUseItemEvent.Finish`: heal the acting player by the tier's amount and hand
    /// back one fewer heart unless they are creative (`ItemHeart.java:55-68`). The engine puts
    /// `event.result` in the hand and syncs it, so no inventory resend is needed.
    fn living_entity_use_item_finish(
        &self,
        host: HostSide,
        event: &mut LivingEntityUseItemFinishEvent<'_>,
    ) {
        let HostSide::Server(host) = host else {
            return;
        };
        // Only a player has the health and game mode this reads; a mob's use has no page in
        // upstream's item.
        if event.entity.is_some() {
            return;
        }
        let stack = event.stack;
        let Some(heart) = MiniHeart::from_item(&stack.item().id(&*host)) else {
            return;
        };
        let Some(player) = host.players().acting_player() else {
            return;
        };
        let Some(living) = host.living(player.entity()) else {
            return;
        };
        let max_health = living.max_health(&*host).unwrap_or(20.0);
        let health = living.health(&*host).unwrap_or(max_health);
        let mode = if player.game_mode(&*host) == Some(GameMode::Creative) {
            PlayerMode::Creative
        } else {
            PlayerMode::Other
        };
        let Some(outcome) = use_heart(heart, health, max_health, mode, &self.config()) else {
            return;
        };
        if (outcome.health - health).abs() > f32::EPSILON {
            living.set_health(host, outcome.health);
        }
        if outcome.consume {
            *event.result = one_less(stack);
        }
    }

    /// A mob's drops are rolled: add the configured miniature hearts from upstream's
    /// `drops.json` defaults (`DropHandler.java:32-119`). Only a mob death's loot is touched: a
    /// block break names no mob, a player's use sets `used`, and a leashed mob's lead drop names
    /// the mob without a death's damage source.
    fn loot_drops(&self, host: &mut dyn ServerHost, event: &mut LootDropsEvent<'_>) {
        let context = event.context;
        let Some((entity, kind)) = context.entity else {
            return;
        };
        if drop_call(true, context.used, context.damage_source.is_some()) != DropCall::MobDeath {
            return;
        }
        let config = self.config();
        if !config.mob_drops || config.heart_drop_amount == 0 {
            return;
        }
        let category = host
            .entity(entity)
            .and_then(|dead| dead.classification(&*host))
            .map_or("", category_name);
        let Some(level) = host.level(context.dimension) else {
            return;
        };
        let drawn = level.with_random(&mut *host, |random| {
            heart_drops(kind, category, &config, || {
                u32::try_from(random.next_int_bound(100).unwrap_or(0)).unwrap_or(0)
            })
        });
        let Some(drops) = drawn else {
            return;
        };
        event.drops.extend(
            drops
                .into_iter()
                .filter_map(|drop| item_stack(&*host, drop.heart.item_id(), drop.count)),
        );
    }
}

/// The 30-tick chew and the eat pose of one miniature heart (`ItemHeart.java:38-52`). One Part
/// carries both, because an item takes one Part of a kind.
struct MiniHeartPart {
    heart: MiniHeart,
    state: Shared,
}

#[mod_api::horizon_part]
impl ItemPart for MiniHeartPart {
    /// `Item.getUseDuration`: 30 ticks while `heart_eating` is on; with it off the heart starts
    /// no use at all, so it is inert without a veto.
    fn use_duration(&self, _host: &dyn Host, event: &mut UseDurationEvent<'_>) {
        let config = lock(&self.state).config();
        if let Some(ticks) = heart_use_duration(self.heart, &config) {
            event.duration = ticks;
        }
    }

    /// `Item.getUseAnimation`: the first-person eat pose (`UseAnim.EAT`).
    fn use_animation(&self, _host: &dyn ClientHost, event: &mut UseAnimationEvent<'_>) {
        *event.animation = UseAnimation::Eat;
    }
}

/// The canisters one player carries, from their main inventory and off hand. The server's slot
/// list is the authority for a host and a guest alike.
fn carried_canisters(host: &dyn Host, player: PlayerRef) -> Option<Carried> {
    let slots = player.inventory_slots(host)?;
    let items: Vec<(String, u32)> = slots
        .iter()
        .filter(|(slot, _)| COUNTED_SLOTS.contains(slot) || *slot == OFF_HAND_SLOT)
        .map(|(_, stack)| (stack.item().id(host), stack.count()))
        .collect();
    Some(Carried::from_stacks(
        items.iter().map(|(item, count)| (item.as_str(), *count)),
    ))
}

/// Apply (or update) this mod's max-health modifier and keep the current health the same
/// distance below the maximum, as `updatePlayerHealth` does (`HealthModifier.java:149-182`).
/// Replacing by id every tick is idempotent, which is what makes a respawn or a dimension
/// change re-apply it without any state of our own.
fn apply_health(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    carried: &Carried,
    config: &ServerConfig,
) {
    let Some(living) = host.living(player.entity()) else {
        return;
    };
    let Some(old_max) = living.attribute_value(&*host, MAX_HEALTH) else {
        return;
    };
    let old_health = living.health(&*host).unwrap_or(old_max as f32);
    let Some(modifier) = health_modifier(f64::from(carried.health_bonus(config))) else {
        return;
    };
    if !living.replace_attribute_modifier(host, MAX_HEALTH, modifier, ModifierLifetime::Transient) {
        return;
    }
    let new_max = living
        .attribute_value(&*host, MAX_HEALTH)
        .unwrap_or(old_max) as f32;
    let new_health = next_health(old_health, old_max as f32, new_max);
    if (new_health - old_health).abs() > f32::EPSILON {
        living.set_health(host, new_health);
    }
}

/// Upstream's modifier (`HealthModifier.java:19,171`): `bhc:extra_health`, `ADD_VALUE`.
fn health_modifier(amount: f64) -> Option<AttributeModifier> {
    let id = Identifier::parse(HEALTH_MODIFIER_ID).ok()?;
    Some(AttributeModifier::new(
        id,
        amount,
        AttributeOperation::AddValue,
    ))
}

/// The stack after eating one of it: `None` when the last one goes (`stack.consume(1)`).
fn one_less(stack: &ItemStack) -> Option<ItemStack> {
    stack.clone().shrink(1)
}

/// A default-component stack of `count` `item`, the shape upstream's `new ItemStack(heart)` has.
fn item_stack(host: &dyn Host, item: &str, count: u32) -> Option<ItemStack> {
    let id = Identifier::parse(item).ok()?;
    let key: ItemKey = host.item_key(&id)?;
    ItemStack::new(key, count)
}

/// The category name the pure core reads. It tests `monster` and nothing else
/// (`runtime::class_of`), as upstream's `instanceof Monster` does (`DropHandler.java:88-109`).
fn category_name(category: MobCategory) -> &'static str {
    match category {
        MobCategory::Monster => "monster",
        _ => "other",
    }
}

/// Which `loot_drops` call this is. Upstream's table answers a mob's death and nothing else:
/// a player's use (shears, a brush) sets `used`; a block break names no mob; and a leashed mob's
/// lead drop names the mob without a death's damage source (`LootDrops::damage_source` is
/// "None for non-death drops").
///
/// The mob's own loot table is **not** a test: `LootDrops::table` is empty when the mob has
/// none, and the ender dragon's is empty — yet the dragon is one of upstream's four shipped
/// entries (`BaubleyHeartCanisters.java:75-78`), so an empty table must still roll the green
/// heart. The e2e's dragon leg is what caught this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropCall {
    /// A mob died: the only call the heart table answers.
    MobDeath,
    /// A block break, a player's use, or a leashed mob's lead drop.
    NotADeath,
}

fn drop_call(mob: bool, used: bool, death: bool) -> DropCall {
    if mob && !used && death {
        DropCall::MobDeath
    } else {
        DropCall::NotADeath
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modifier_is_upstreams_id_and_operation() {
        let modifier = health_modifier(20.0).expect("bhc:extra_health is a valid identifier");
        assert_eq!(
            modifier.id,
            Identifier::parse("bhc:extra_health").expect("the id parses")
        );
        assert_eq!(modifier.operation, AttributeOperation::AddValue);
        assert_eq!(modifier.amount, 20.0);
    }

    #[test]
    fn only_monsters_are_hostile_to_the_drop_table() {
        assert_eq!(category_name(MobCategory::Monster), "monster");
        for category in [
            MobCategory::Creature,
            MobCategory::Ambient,
            MobCategory::WaterCreature,
        ] {
            assert_ne!(category_name(category), "monster", "{category:?}");
        }
    }

    #[test]
    fn a_mob_death_is_the_only_call_the_drop_table_answers() {
        assert_eq!(drop_call(true, false, true), DropCall::MobDeath);
        // A leashed mob's lead drop names the mob but is not a death.
        assert_eq!(drop_call(true, false, false), DropCall::NotADeath);
        // A player's use (shears, a brush) sets `used`.
        assert_eq!(drop_call(true, true, true), DropCall::NotADeath);
        // A block break names no mob.
        assert_eq!(drop_call(false, false, true), DropCall::NotADeath);
    }
}
