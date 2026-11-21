// Baubley Heart Canisters for Horizon — the pure health core.
//
// Ported from Baubley Heart Canisters 1.21.1-1.4.3
// (Traverse-Joe/Baubley-Heart-Canisters, MIT) for Minecraft 1.21.1 NeoForge.
// Upstream source read for behaviour only; this is a clean-room Rust rewrite.
//
// Upstream stores one canister stack per colour in the Heart Amulet's
// `bhc:hearts` component and grants 2 health points per stored canister
// (`HealthModifier.java:161,184-206`). Slice 1 defers Curios and the amulet:
// the player's carried main inventory stands in for the amulet slots. Each
// colour is capped by upstream's `heartStackSize` (default 10,
// `ConfigHandler.java:27-29`). Slice 3 adds the miniature hearts' own use:
// eating one heals `HeartType.healAmount` (red 20, yellow 40, green 60, blue
// 80, `HeartType.java:4-8`, `ItemHeart.java:55-68`), clamped to max health,
// and consumes one unless the player is creative. Miniature hearts never grant
// max health; only the canisters do, through the amulet formula above.
// Nothing here touches the game; the glue scans the inventory and applies the
// modifier, and calls [`use_heart`] on a heart's right click.

/// Upstream's modifier id (`HealthModifier.java:19`).
pub const HEALTH_MODIFIER_ID: &str = "bhc:extra_health";

/// Upstream `heartStackSize` default and allowed range
/// (`ConfigHandler.java:27-29`).
pub const HEART_STACK_SIZE_DEFAULT: u32 = 10;
pub const HEART_STACK_SIZE_MAX: u32 = 99;

/// Health points one stored canister grants (`HealthModifier.java:161`).
pub const HEALTH_PER_CANISTER: i32 = 2;

/// Upstream's `drops.json` defaults (`BaubleyHeartCanisters.java:75-78`):
/// red from hostile mobs, yellow from bosses, green from the dragon, blue from
/// the warden. Each entry is one miniature heart with a per-entry chance.
pub const RED_HEART_CHANCE_DEFAULT: f64 = 0.15;
pub const YELLOW_HEART_CHANCE_DEFAULT: f64 = 1.0;
pub const GREEN_HEART_CHANCE_DEFAULT: f64 = 1.0;
pub const BLUE_HEART_CHANCE_DEFAULT: f64 = 1.0;

/// One `ItemStack(heart)` per successful entry (`DropHandler.java:114-119`).
pub const HEART_DROP_AMOUNT_DEFAULT: u32 = 1;
pub const HEART_DROP_AMOUNT_MAX: u32 = 64;

/// Upstream `HeartType`'s heal amounts (`HeartType.java:4-8`): eating a
/// miniature heart restores these health points
/// (`ItemHeart.java:55-68`).
pub const RED_HEART_HEAL: i32 = 20;
pub const YELLOW_HEART_HEAL: i32 = 40;
pub const GREEN_HEART_HEAL: i32 = 60;
pub const BLUE_HEART_HEAL: i32 = 80;

/// Upstream's `ItemHeart.getUseDuration` (`ItemHeart.java:37-53`): a
/// miniature heart is eaten over 30 ticks. The pin's `use_duration` hook
/// (API 90) is what makes the chew real instead of an instant use.
pub const HEART_USE_TICKS: i32 = 30;

/// The port's upper bound for a configured heal amount. Upstream hardcodes
/// the four values; this cap only keeps a typo from healing absurdly.
pub const MINI_HEART_HEAL_MAX: i32 = 1000;

/// Where the glue keeps the server settings (upstream uses a NeoForge config).
pub const CONFIG_FILE: &str = "bhc.conf";

/// The five canister colours, in upstream's slot order
/// (`HeartAmuletContainer.java:17-21`, `SoulHeartAmuletContainer.java:17-21`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Red,
    Yellow,
    Green,
    Blue,
    Soul,
}

impl Tier {
    /// Every canister tier, upstream slot order.
    pub const ALL: [Tier; 5] = [Tier::Red, Tier::Yellow, Tier::Green, Tier::Blue, Tier::Soul];

    /// The registry id of this tier's canister.
    pub fn item_id(self) -> &'static str {
        match self {
            Tier::Red => "bhc:red_heart_canister",
            Tier::Yellow => "bhc:yellow_heart_canister",
            Tier::Green => "bhc:green_heart_canister",
            Tier::Blue => "bhc:blue_heart_canister",
            Tier::Soul => "bhc:soul_heart_canister",
        }
    }

    /// The tier a registry id names, or None for any other item.
    pub fn from_item(item: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|tier| tier.item_id() == item)
    }

    /// The tier's index in [`Tier::ALL`].
    pub fn index(self) -> usize {
        match self {
            Tier::Red => 0,
            Tier::Yellow => 1,
            Tier::Green => 2,
            Tier::Blue => 3,
            Tier::Soul => 4,
        }
    }
}

/// The four miniature hearts upstream drops from mobs
/// (`RegistryHandler.java:32-35`, `DropHandler.java:58-65`). Soul has no
/// miniature item, so it is not a drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MiniHeart {
    Red,
    Yellow,
    Green,
    Blue,
}

impl MiniHeart {
    /// Every droppable heart, upstream's `getEntityDrops` order.
    pub const ALL: [MiniHeart; 4] = [
        MiniHeart::Red,
        MiniHeart::Yellow,
        MiniHeart::Green,
        MiniHeart::Blue,
    ];

    /// The registry id of this heart.
    pub fn item_id(self) -> &'static str {
        match self {
            MiniHeart::Red => "bhc:red_heart",
            MiniHeart::Yellow => "bhc:yellow_heart",
            MiniHeart::Green => "bhc:green_heart",
            MiniHeart::Blue => "bhc:blue_heart",
        }
    }

    /// The heart a registry id names, or None for any other item.
    pub fn from_item(item: &str) -> Option<MiniHeart> {
        MiniHeart::ALL
            .into_iter()
            .find(|heart| heart.item_id() == item)
    }

    /// The health points upstream's `HeartType.healAmount` restores
    /// (`HeartType.java:4-8`).
    pub fn heal_amount(self) -> i32 {
        match self {
            MiniHeart::Red => RED_HEART_HEAL,
            MiniHeart::Yellow => YELLOW_HEART_HEAL,
            MiniHeart::Green => GREEN_HEART_HEAL,
            MiniHeart::Blue => BLUE_HEART_HEAL,
        }
    }

    /// The `config/bhc.conf` key for this heart's configured heal amount.
    pub fn heal_config_key(self) -> &'static str {
        match self {
            MiniHeart::Red => "red_heart_heal",
            MiniHeart::Yellow => "yellow_heart_heal",
            MiniHeart::Green => "green_heart_heal",
            MiniHeart::Blue => "blue_heart_heal",
        }
    }

    /// The heart a heal config key names, or None for another key.
    pub fn from_heal_config_key(key: &str) -> Option<MiniHeart> {
        MiniHeart::ALL
            .into_iter()
            .find(|heart| heart.heal_config_key() == key)
    }

    /// The `config/bhc.conf` key for this heart's chance.
    pub fn config_key(self) -> &'static str {
        match self {
            MiniHeart::Red => "red_heart_drop_chance",
            MiniHeart::Yellow => "yellow_heart_drop_chance",
            MiniHeart::Green => "green_heart_drop_chance",
            MiniHeart::Blue => "blue_heart_drop_chance",
        }
    }

    /// The heart a config key names, or None for another key.
    pub fn from_config_key(key: &str) -> Option<MiniHeart> {
        MiniHeart::ALL
            .into_iter()
            .find(|heart| heart.config_key() == key)
    }

    /// The heart's index in [`MiniHeart::ALL`].
    pub fn index(self) -> usize {
        match self {
            MiniHeart::Red => 0,
            MiniHeart::Yellow => 1,
            MiniHeart::Green => 2,
            MiniHeart::Blue => 3,
        }
    }

    /// The entity group upstream's default `drops.json` ties this heart to
    /// (`BaubleyHeartCanisters.java:75-78`).
    pub fn default_group(self) -> DropGroup {
        match self {
            MiniHeart::Red => DropGroup::Hostile,
            MiniHeart::Yellow => DropGroup::Boss,
            MiniHeart::Green => DropGroup::Dragon,
            MiniHeart::Blue => DropGroup::Warden,
        }
    }
}

/// The entity groups upstream's default `drops.json` names
/// (`DropHandler.java:88-109`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropGroup {
    /// A monster that is not a boss (`Monster`, except bosses but the warden).
    Hostile,
    /// A boss tag member other than the dragon.
    Boss,
    /// The ender dragon.
    Dragon,
    /// `minecraft:warden`, upstream's explicit blue-heart entry.
    Warden,
}

/// What kind of entity a mob kind is for upstream's drop groups
/// (`DropHandler.java:88-109`). The pin names mobs by kind, so the glue reads
/// the entity type's category (`monster`, `creature`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobClass {
    Passive,
    Hostile,
    Boss,
    Dragon,
    Other,
}

/// Classify a mob by its id and its entity type's category. Upstream's boss
/// set is the `c:bosses` tag: the wither, the warden and the ender dragon;
/// the dragon is considered separately and the warden still counts as a
/// monster (`DropHandler.java:94-107`).
#[must_use]
pub fn class_of(kind: &str, category: &str) -> MobClass {
    match kind {
        "minecraft:ender_dragon" => MobClass::Dragon,
        "minecraft:wither" | "minecraft:warden" => MobClass::Boss,
        "minecraft:player" => MobClass::Other,
        _ if category == "monster" => MobClass::Hostile,
        _ => MobClass::Passive,
    }
}

/// Whether a mob belongs to the group an entry names. The warden is both a
/// monster (red) and a boss (yellow) upstream, and its explicit blue entry
/// always matches (`DropHandler.java:85-109`).
#[must_use]
pub fn group_matches(group: DropGroup, kind: &str, class: MobClass) -> bool {
    match group {
        DropGroup::Hostile => class == MobClass::Hostile || kind == "minecraft:warden",
        DropGroup::Boss => class == MobClass::Boss,
        DropGroup::Dragon => class == MobClass::Dragon,
        DropGroup::Warden => kind == "minecraft:warden",
    }
}

/// Upstream's `addWithPercent`: `(int)(chance * 100)` and
/// `random.nextInt(100) < percent` (`DropHandler.java:114-119`). The caller
/// supplies the 0..100 draw, so tests are deterministic.
#[must_use]
pub fn roll(chance: f64, next_percent: u32) -> bool {
    let chance = if chance.is_finite() {
        chance.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let percent = (chance * 100.0) as u32;
    next_percent < percent
}

/// What upstream's `Player.isCreative()` decides for a finished heart
/// (`ItemHeart.java:57-59`): a creative player keeps the item, everyone else
/// consumes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerMode {
    Creative,
    Other,
}

impl PlayerMode {
    /// The mode of a game-mode number (0 survival, 1 creative, 2 adventure,
    /// 3 spectator).
    #[must_use]
    pub fn of(game_mode: i32) -> Self {
        if game_mode == 1 {
            PlayerMode::Creative
        } else {
            PlayerMode::Other
        }
    }

    /// Whether a finished use consumes one of the item.
    #[must_use]
    pub fn consumes(self) -> bool {
        self == PlayerMode::Other
    }
}

/// `LivingEntity.heal(amount)` as upstream uses it: add the health points and
/// clamp to the current maximum. Never touches the maximum itself.
#[must_use]
pub fn heal_health(health: f32, max_health: f32, amount: i32) -> f32 {
    let max = max_health.max(0.0);
    (health.max(0.0) + amount as f32).clamp(0.0, max)
}

/// What eating one miniature heart does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeartUse {
    /// The health after `heal_health`.
    pub health: f32,
    /// Whether one of the item leaves the inventory.
    pub consume: bool,
}

/// One miniature heart's use, or None when the port's `heart_eating` switch is
/// off. Upstream heals `HeartType.healAmount` and consumes one unless the
/// player is creative (`ItemHeart.java:55-68`); it grants no permanent hearts.
#[must_use]
pub fn use_heart(
    heart: MiniHeart,
    health: f32,
    max_health: f32,
    mode: PlayerMode,
    config: &ServerConfig,
) -> Option<HeartUse> {
    if !config.heart_eating {
        return None;
    }
    Some(HeartUse {
        health: heal_health(health, max_health, config.heart_heal(heart)),
        consume: mode.consumes(),
    })
}

/// How long a miniature heart is used for, or None when `heart_eating` is off
/// (the item then starts no use at all). The `heart` argument is the item the
/// caller matched; it keeps the call site self-documenting and leaves room for
/// per-tier use times upstream does not have today.
#[must_use]
pub fn heart_use_duration(_heart: MiniHeart, config: &ServerConfig) -> Option<i32> {
    config.heart_eating.then_some(HEART_USE_TICKS)
}

/// One heart entry that dropped, and how many of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeartDrop {
    pub heart: MiniHeart,
    pub count: u32,
}

/// The hearts a mob of `kind`/`category` drops under `config`. One roll per
/// heart entry whose group matches, in upstream's red/yellow/green/blue order.
/// `next_percent` is called once per matching entry with upstream's bound 100.
#[must_use]
pub fn heart_drops(
    kind: &str,
    category: &str,
    config: &ServerConfig,
    mut next_percent: impl FnMut() -> u32,
) -> Vec<HeartDrop> {
    if !config.mob_drops || config.heart_drop_amount == 0 {
        return Vec::new();
    }
    let class = class_of(kind, category);
    let mut drops = Vec::new();
    for heart in MiniHeart::ALL {
        if group_matches(heart.default_group(), kind, class)
            && roll(config.chance(heart), next_percent())
        {
            drops.push(HeartDrop {
                heart,
                count: config.heart_drop_amount,
            });
        }
    }
    drops
}

/// The server settings this slice understands. Defaults are upstream's.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    /// The maximum stack size for heart canisters, and so the maximum number
    /// of canisters of one colour that count (`ConfigHandler.java:27-29`).
    pub heart_stack_size: u32,
    /// Whether mobs drop miniature hearts. Upstream has no switch; this slice
    /// adds one so a server can turn the drop table off. The world's
    /// `/gamerule mob_drops` is a second switch the engine applies by clearing
    /// the loot table; the glue reads it too, because the engine still calls the
    /// drop hook when it is off.
    pub mob_drops: bool,
    /// How many of a heart one successful entry drops.
    pub heart_drop_amount: u32,
    /// Whether miniature hearts can be eaten. Upstream has no switch; this
    /// port adds one so a server can turn the heal off.
    pub heart_eating: bool,
    /// One chance per [`MiniHeart`], indexed by [`MiniHeart::index`].
    chances: [f64; 4],
    /// One heal amount per [`MiniHeart`], indexed by [`MiniHeart::index`].
    heals: [i32; 4],
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            heart_stack_size: HEART_STACK_SIZE_DEFAULT,
            mob_drops: true,
            heart_drop_amount: HEART_DROP_AMOUNT_DEFAULT,
            heart_eating: true,
            chances: [
                RED_HEART_CHANCE_DEFAULT,
                YELLOW_HEART_CHANCE_DEFAULT,
                GREEN_HEART_CHANCE_DEFAULT,
                BLUE_HEART_CHANCE_DEFAULT,
            ],
            heals: [
                MiniHeart::Red.heal_amount(),
                MiniHeart::Yellow.heal_amount(),
                MiniHeart::Green.heal_amount(),
                MiniHeart::Blue.heal_amount(),
            ],
        }
    }
}

impl ServerConfig {
    /// The chance a matching mob drops `heart`.
    #[must_use]
    pub fn chance(&self, heart: MiniHeart) -> f64 {
        self.chances[heart.index()]
    }

    /// Set `heart`'s chance, clamped to 0..=1. A non-finite value turns the
    /// entry off.
    pub fn set_chance(&mut self, heart: MiniHeart, chance: f64) {
        let chance = if chance.is_finite() {
            chance.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.chances[heart.index()] = chance;
    }

    /// The health points eating `heart` restores.
    #[must_use]
    pub fn heart_heal(&self, heart: MiniHeart) -> i32 {
        self.heals[heart.index()]
    }

    /// Set `heart`'s heal amount, clamped to 0..=[`MINI_HEART_HEAL_MAX`].
    pub fn set_heart_heal(&mut self, heart: MiniHeart, amount: i64) {
        self.heals[heart.index()] = amount.clamp(0, i64::from(MINI_HEART_HEAL_MAX)) as i32;
    }
}

/// Parse `config/bhc.conf`: `key = value` lines, `#` comments. Unknown keys and
/// badly typed values keep their defaults; numbers are clamped.
#[must_use]
pub fn parse_config(text: &str) -> ServerConfig {
    let mut config = ServerConfig::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "heart_stack_size" => {
                if let Ok(value) = value.parse::<i64>() {
                    config.heart_stack_size =
                        value.clamp(0, i64::from(HEART_STACK_SIZE_MAX)) as u32;
                }
            }
            "mob_drops" => {
                if let Ok(value) = value.parse::<bool>() {
                    config.mob_drops = value;
                }
            }
            "heart_drop_amount" => {
                if let Ok(value) = value.parse::<i64>() {
                    config.heart_drop_amount =
                        value.clamp(0, i64::from(HEART_DROP_AMOUNT_MAX)) as u32;
                }
            }
            "heart_eating" => {
                if let Ok(value) = value.parse::<bool>() {
                    config.heart_eating = value;
                }
            }
            _ => {
                if let Some(heart) = MiniHeart::from_config_key(key)
                    && let Ok(chance) = value.parse::<f64>()
                {
                    config.set_chance(heart, chance);
                } else if let Some(heart) = MiniHeart::from_heal_config_key(key)
                    && let Ok(amount) = value.parse::<i64>()
                {
                    config.set_heart_heal(heart, amount);
                }
            }
        }
    }
    config
}

/// Write the config in the format `parse_config` reads.
#[must_use]
pub fn format_config(config: &ServerConfig) -> String {
    format!(
        "# Baubley Heart Canisters (Horizon) — server settings.\n\
         # Maximum canisters of one colour that count, and the canister stack size upstream used.\n\
         heart_stack_size = {}\n\
         # Miniature hearts mobs drop (upstream's drops.json defaults).\n\
         # The world's /gamerule mob_drops must be on as well.\n\
         mob_drops = {}\n\
         heart_drop_amount = {}\n\
         red_heart_drop_chance = {}\n\
         yellow_heart_drop_chance = {}\n\
         green_heart_drop_chance = {}\n\
         blue_heart_drop_chance = {}\n\
         # Miniature hearts: eating heals these health points (upstream's HeartType).\n\
         heart_eating = {}\n\
         red_heart_heal = {}\n\
         yellow_heart_heal = {}\n\
         green_heart_heal = {}\n\
         blue_heart_heal = {}\n",
        config.heart_stack_size,
        config.mob_drops,
        config.heart_drop_amount,
        config.chance(MiniHeart::Red),
        config.chance(MiniHeart::Yellow),
        config.chance(MiniHeart::Green),
        config.chance(MiniHeart::Blue),
        config.heart_eating,
        config.heart_heal(MiniHeart::Red),
        config.heart_heal(MiniHeart::Yellow),
        config.heart_heal(MiniHeart::Green),
        config.heart_heal(MiniHeart::Blue),
    )
}

/// A snapshot of the canisters a player carries: one count per tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Carried {
    counts: [u32; 5],
}

impl Carried {
    pub fn new() -> Self {
        Self::default()
    }

    /// Count one stack of `item` if it is a canister. Returns whether it was.
    pub fn add_stack(&mut self, item: &str, count: u32) -> bool {
        let Some(tier) = Tier::from_item(item) else {
            return false;
        };
        let slot = &mut self.counts[tier.index()];
        *slot = slot.saturating_add(count);
        true
    }

    /// Build a loadout from `(item id, stack count)` pairs, ignoring anything
    /// that is not a heart canister.
    #[must_use]
    pub fn from_stacks<'a>(stacks: impl IntoIterator<Item = (&'a str, u32)>) -> Self {
        let mut carried = Self::new();
        for (item, count) in stacks {
            carried.add_stack(item, count);
        }
        carried
    }

    /// The carried count of one tier.
    #[must_use]
    pub fn count(&self, tier: Tier) -> u32 {
        self.counts[tier.index()]
    }

    /// The counted canisters of one tier after the per-colour cap.
    #[must_use]
    pub fn capped_count(&self, tier: Tier, cap: u32) -> u32 {
        self.count(tier).min(cap)
    }

    /// All counted canisters, each colour capped by `config.heart_stack_size`.
    #[must_use]
    pub fn capped_total(&self, config: &ServerConfig) -> u32 {
        Tier::ALL
            .into_iter()
            .map(|tier| self.capped_count(tier, config.heart_stack_size))
            .sum()
    }

    /// The max-health bonus in health points (`HealthModifier.java:161`).
    #[must_use]
    pub fn health_bonus(&self, config: &ServerConfig) -> i32 {
        let total = self.capped_total(config);
        i32::try_from(total)
            .unwrap_or(i32::MAX)
            .saturating_mul(HEALTH_PER_CANISTER)
    }
}

/// Upstream keeps the current health at the same gap below max
/// (`HealthModifier.java:155,172-180`): health moves by the max-health delta,
/// clamped to the valid range.
#[must_use]
pub fn next_health(old_health: f32, old_max: f32, new_max: f32) -> f32 {
    (old_health + (new_max - old_max)).clamp(0.0, new_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ServerConfig {
        ServerConfig::default()
    }

    #[test]
    fn defaults_match_upstream() {
        let c = config();
        assert_eq!(c.heart_stack_size, HEART_STACK_SIZE_DEFAULT);
        assert_eq!(c.heart_stack_size, 10);
        assert!(c.mob_drops);
        assert_eq!(c.heart_drop_amount, 1);
        assert_eq!(c.chance(MiniHeart::Red), 0.15);
        assert_eq!(c.chance(MiniHeart::Yellow), 1.0);
        assert_eq!(c.chance(MiniHeart::Green), 1.0);
        assert_eq!(c.chance(MiniHeart::Blue), 1.0);
    }

    #[test]
    fn config_parses_and_clamps() {
        let c = parse_config(
            "# server settings\n\
             heart_stack_size = 4\n\
             unknown = 9\n",
        );
        assert_eq!(c.heart_stack_size, 4);
        assert_eq!(
            parse_config("heart_stack_size = nope\n").heart_stack_size,
            10
        );
        assert_eq!(parse_config("heart_stack_size = -3\n").heart_stack_size, 0);
        assert_eq!(
            parse_config("heart_stack_size = 5000\n").heart_stack_size,
            HEART_STACK_SIZE_MAX
        );
        assert_eq!(parse_config("").heart_stack_size, 10);
    }

    #[test]
    fn config_round_trips_through_the_written_defaults() {
        let c = ServerConfig::default();
        assert_eq!(parse_config(&format_config(&c)), c);
        let c = parse_config("heart_stack_size = 3\n");
        assert_eq!(parse_config(&format_config(&c)), c);
    }

    #[test]
    fn tier_ids_map_exactly() {
        assert_eq!(Tier::from_item("bhc:red_heart_canister"), Some(Tier::Red));
        assert_eq!(Tier::from_item("bhc:soul_heart_canister"), Some(Tier::Soul));
        for tier in Tier::ALL {
            assert_eq!(Tier::from_item(tier.item_id()), Some(tier), "{tier:?}");
        }
        for other in [
            "bhc:red_heart",
            "bhc:canister",
            "minecraft:stick",
            "bhc:red_heart_canister2",
            "",
        ] {
            assert_eq!(Tier::from_item(other), None, "{other}");
        }
    }

    #[test]
    fn one_canister_is_one_heart() {
        let c = config();
        let carried = Carried::from_stacks([("bhc:red_heart_canister", 1)]);
        assert_eq!(carried.count(Tier::Red), 1);
        assert_eq!(carried.capped_total(&c), 1);
        assert_eq!(carried.health_bonus(&c), 2);
    }

    #[test]
    fn two_canisters_add_four_health() {
        let c = config();
        let carried = Carried::from_stacks([("bhc:red_heart_canister", 2)]);
        assert_eq!(carried.health_bonus(&c), 4);
    }

    #[test]
    fn every_colour_counts_including_soul() {
        let c = config();
        let carried = Carried::from_stacks([
            ("bhc:red_heart_canister", 1),
            ("bhc:yellow_heart_canister", 1),
            ("bhc:green_heart_canister", 1),
            ("bhc:blue_heart_canister", 1),
            ("bhc:soul_heart_canister", 1),
        ]);
        assert_eq!(carried.capped_total(&c), 5);
        assert_eq!(carried.health_bonus(&c), 10);
    }

    #[test]
    fn each_colour_is_capped_by_the_config() {
        let c = config();
        let carried = Carried::from_stacks([
            ("bhc:red_heart_canister", 12),
            ("bhc:blue_heart_canister", 3),
        ]);
        assert_eq!(carried.capped_count(Tier::Red, c.heart_stack_size), 10);
        assert_eq!(carried.capped_count(Tier::Blue, c.heart_stack_size), 3);
        assert_eq!(carried.capped_total(&c), 13);
        assert_eq!(carried.health_bonus(&c), 26);
    }

    #[test]
    fn multiple_stacks_of_one_colour_share_the_cap() {
        let c = config();
        let carried =
            Carried::from_stacks([("bhc:red_heart_canister", 7), ("bhc:red_heart_canister", 5)]);
        assert_eq!(carried.count(Tier::Red), 12);
        assert_eq!(carried.capped_total(&c), 10);
        assert_eq!(carried.health_bonus(&c), 20);
    }

    #[test]
    fn a_smaller_config_cap_reduces_the_bonus() {
        let c = parse_config("heart_stack_size = 3\n");
        let carried = Carried::from_stacks([("bhc:red_heart_canister", 8)]);
        assert_eq!(carried.health_bonus(&c), 6);
    }

    #[test]
    fn a_zero_cap_turns_the_bonus_off() {
        let c = parse_config("heart_stack_size = 0\n");
        let carried = Carried::from_stacks([("bhc:red_heart_canister", 8)]);
        assert_eq!(carried.capped_total(&c), 0);
        assert_eq!(carried.health_bonus(&c), 0);
    }

    #[test]
    fn non_canister_items_are_ignored() {
        let c = config();
        let mut carried = Carried::new();
        assert!(!carried.add_stack("minecraft:stick", 64));
        assert!(!carried.add_stack("bhc:red_heart", 4));
        assert!(!carried.add_stack("bhc:canister", 1));
        assert!(carried.add_stack("bhc:red_heart_canister", 2));
        assert_eq!(carried.health_bonus(&c), 4);
    }

    #[test]
    fn health_keeps_the_gap_below_max_on_gain() {
        assert_eq!(next_health(20.0, 20.0, 24.0), 24.0);
        assert_eq!(next_health(15.0, 20.0, 24.0), 19.0);
        assert_eq!(next_health(24.0, 20.0, 24.0), 24.0);
    }

    #[test]
    fn health_keeps_the_gap_below_max_on_loss() {
        assert_eq!(next_health(24.0, 24.0, 20.0), 20.0);
        assert_eq!(next_health(22.0, 24.0, 20.0), 18.0);
    }

    #[test]
    fn health_is_clamped_to_the_new_range() {
        assert_eq!(next_health(0.0, 20.0, 4.0), 0.0);
        assert_eq!(next_health(2.0, 20.0, 4.0), 0.0);
        assert_eq!(next_health(50.0, 20.0, 24.0), 24.0);
        assert_eq!(next_health(19.5, 20.0, 24.0), 23.5);
    }

    #[test]
    fn modifier_id_matches_upstream() {
        assert_eq!(HEALTH_MODIFIER_ID, "bhc:extra_health");
    }

    #[test]
    fn mob_drop_config_parses_and_clamps() {
        let c = parse_config(
            "mob_drops = false\n\
             heart_drop_amount = 3\n\
             red_heart_drop_chance = 0.5\n\
             blue_heart_drop_chance = 4.0\n\
             green_heart_drop_chance = -1\n\
             yellow_heart_drop_chance = nope\n",
        );
        assert!(!c.mob_drops);
        assert_eq!(c.heart_drop_amount, 3);
        assert_eq!(c.chance(MiniHeart::Red), 0.5);
        assert_eq!(c.chance(MiniHeart::Blue), 1.0);
        assert_eq!(c.chance(MiniHeart::Green), 0.0);
        assert_eq!(c.chance(MiniHeart::Yellow), 1.0);

        let d = parse_config(
            "mob_drops = maybe\n\
             heart_drop_amount = -1\n\
             red_heart_drop_chance = 0.25\n",
        );
        assert!(d.mob_drops);
        assert_eq!(d.heart_drop_amount, 0);
        assert_eq!(d.chance(MiniHeart::Red), 0.25);
        assert_eq!(
            parse_config("heart_drop_amount = 5000\n").heart_drop_amount,
            HEART_DROP_AMOUNT_MAX
        );
    }

    #[test]
    fn config_round_trips_mob_drop_settings() {
        let c = parse_config(
            "mob_drops = false\n\
             heart_drop_amount = 2\n\
             red_heart_drop_chance = 0.33\n",
        );
        assert_eq!(parse_config(&format_config(&c)), c);
    }

    #[test]
    fn miniature_heart_ids_match_upstream() {
        assert_eq!(MiniHeart::Red.item_id(), "bhc:red_heart");
        assert_eq!(MiniHeart::Yellow.item_id(), "bhc:yellow_heart");
        assert_eq!(MiniHeart::Green.item_id(), "bhc:green_heart");
        assert_eq!(MiniHeart::Blue.item_id(), "bhc:blue_heart");
    }

    #[test]
    fn mob_classes_follow_upstream_categories_and_bosses() {
        assert_eq!(class_of("minecraft:zombie", "monster"), MobClass::Hostile);
        assert_eq!(class_of("minecraft:cow", "creature"), MobClass::Passive);
        assert_eq!(
            class_of("minecraft:armadillo", "creature"),
            MobClass::Passive
        );
        assert_eq!(class_of("minecraft:wither", "monster"), MobClass::Boss);
        assert_eq!(class_of("minecraft:warden", "monster"), MobClass::Boss);
        assert_eq!(
            class_of("minecraft:ender_dragon", "monster"),
            MobClass::Dragon
        );
        assert_eq!(class_of("minecraft:player", "misc"), MobClass::Other);
    }

    #[test]
    fn default_groups_match_upstream_drops_json() {
        assert_eq!(MiniHeart::Red.default_group(), DropGroup::Hostile);
        assert_eq!(MiniHeart::Yellow.default_group(), DropGroup::Boss);
        assert_eq!(MiniHeart::Green.default_group(), DropGroup::Dragon);
        assert_eq!(MiniHeart::Blue.default_group(), DropGroup::Warden);
    }

    #[test]
    fn chance_rolls_use_upstream_truncated_percent() {
        assert!(!roll(0.0, 0));
        assert!(roll(0.15, 14));
        assert!(!roll(0.15, 15));
        assert!(roll(0.155, 14));
        assert!(!roll(0.155, 15));
        assert!(roll(1.0, 99));
        assert!(!roll(f64::NAN, 0));
    }

    fn all_mobs_config(amount: u32) -> ServerConfig {
        parse_config(&format!(
            "mob_drops = true\n\
             heart_drop_amount = {amount}\n\
             red_heart_drop_chance = 1.0\n\
             yellow_heart_drop_chance = 1.0\n\
             green_heart_drop_chance = 1.0\n\
             blue_heart_drop_chance = 1.0\n"
        ))
    }

    fn percents(values: &[u32]) -> impl FnMut() -> u32 + '_ {
        let mut values = values.iter().copied();
        move || values.next().unwrap_or(99)
    }

    #[test]
    fn a_zombie_drops_the_configured_red_hearts() {
        let c = all_mobs_config(2);
        let drops = heart_drops("minecraft:zombie", "monster", &c, percents(&[0]));
        assert_eq!(
            drops,
            vec![HeartDrop {
                heart: MiniHeart::Red,
                count: 2,
            }]
        );
    }

    #[test]
    fn a_cow_drops_no_hearts_and_rolls_nothing() {
        let c = all_mobs_config(1);
        let mut rolled = false;
        let drops = heart_drops("minecraft:cow", "creature", &c, || {
            rolled = true;
            0
        });
        assert!(drops.is_empty());
        assert!(!rolled);
    }

    #[test]
    fn a_dragon_drops_only_the_green_heart() {
        let c = all_mobs_config(1);
        let drops = heart_drops("minecraft:ender_dragon", "monster", &c, percents(&[0]));
        assert_eq!(
            drops,
            vec![HeartDrop {
                heart: MiniHeart::Green,
                count: 1,
            }]
        );
    }

    #[test]
    fn a_wither_drops_only_the_yellow_heart() {
        let c = all_mobs_config(1);
        let drops = heart_drops("minecraft:wither", "monster", &c, percents(&[0]));
        assert_eq!(
            drops,
            vec![HeartDrop {
                heart: MiniHeart::Yellow,
                count: 1,
            }]
        );
    }

    #[test]
    fn a_warden_drops_red_yellow_and_blue_like_the_upstream_entries() {
        let c = all_mobs_config(1);
        let drops = heart_drops("minecraft:warden", "monster", &c, percents(&[0, 0, 0]));
        assert_eq!(
            drops,
            vec![
                HeartDrop {
                    heart: MiniHeart::Red,
                    count: 1,
                },
                HeartDrop {
                    heart: MiniHeart::Yellow,
                    count: 1,
                },
                HeartDrop {
                    heart: MiniHeart::Blue,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn a_fifteen_percent_chance_drops_below_fifteen_only() {
        let c = parse_config("red_heart_drop_chance = 0.15\n");
        assert_eq!(
            heart_drops("minecraft:zombie", "monster", &c, percents(&[14])).len(),
            1
        );
        assert!(heart_drops("minecraft:zombie", "monster", &c, percents(&[15])).is_empty());
    }

    #[test]
    fn a_disabled_config_drops_nothing_and_rolls_nothing() {
        let c = parse_config(
            "mob_drops = false\n\
             red_heart_drop_chance = 1.0\n",
        );
        let mut rolled = false;
        let drops = heart_drops("minecraft:zombie", "monster", &c, || {
            rolled = true;
            0
        });
        assert!(drops.is_empty());
        assert!(!rolled);
    }

    #[test]
    fn a_zero_amount_drops_nothing() {
        let c = all_mobs_config(0);
        let drops = heart_drops("minecraft:zombie", "monster", &c, percents(&[0]));
        assert!(drops.is_empty());
    }

    #[test]
    fn miniature_hearts_heal_upstream_amounts() {
        let c = config();
        assert_eq!(MiniHeart::Red.heal_amount(), 20);
        assert_eq!(MiniHeart::Yellow.heal_amount(), 40);
        assert_eq!(MiniHeart::Green.heal_amount(), 60);
        assert_eq!(MiniHeart::Blue.heal_amount(), 80);
        assert_eq!(c.heart_heal(MiniHeart::Red), 20);
        assert_eq!(c.heart_heal(MiniHeart::Yellow), 40);
        assert_eq!(c.heart_heal(MiniHeart::Green), 60);
        assert_eq!(c.heart_heal(MiniHeart::Blue), 80);
    }

    #[test]
    fn heal_config_parses_clamps_and_round_trips() {
        let c = parse_config(
            "heart_eating = false\n\
             red_heart_heal = 3\n\
             blue_heart_heal = -4\n\
             green_heart_heal = 5000\n\
             yellow_heart_heal = nope\n",
        );
        assert!(!c.heart_eating);
        assert_eq!(c.heart_heal(MiniHeart::Red), 3);
        assert_eq!(c.heart_heal(MiniHeart::Blue), 0);
        assert_eq!(c.heart_heal(MiniHeart::Green), MINI_HEART_HEAL_MAX);
        assert_eq!(c.heart_heal(MiniHeart::Yellow), 40);
        assert_eq!(parse_config(&format_config(&c)), c);
        assert!(parse_config("heart_eating = maybe\n").heart_eating);
    }

    #[test]
    fn mini_heart_ids_are_not_canisters() {
        for heart in MiniHeart::ALL {
            assert_eq!(MiniHeart::from_item(heart.item_id()), Some(heart));
            assert_eq!(Tier::from_item(heart.item_id()), None);
        }
        for other in [
            "bhc:red_heart_canister",
            "bhc:soul_heart_canister",
            "bhc:canister",
            "minecraft:stick",
            "bhc:red_heart2",
            "",
        ] {
            assert_eq!(MiniHeart::from_item(other), None, "{other}");
        }
    }

    #[test]
    fn healing_adds_the_amount_and_clamps_to_max() {
        assert_eq!(heal_health(10.0, 20.0, 20), 20.0);
        assert_eq!(heal_health(2.0, 40.0, 20), 22.0);
        assert_eq!(heal_health(20.0, 20.0, 20), 20.0);
        assert_eq!(heal_health(0.0, 20.0, 0), 0.0);
        assert_eq!(heal_health(5.0, 100.0, 1000), 100.0);
    }

    #[test]
    fn eating_a_heart_heals_but_never_raises_max_health() {
        let c = config();
        let outcome = use_heart(MiniHeart::Red, 10.0, 20.0, PlayerMode::Other, &c)
            .expect("hearts are edible by default");
        assert_eq!(outcome.health, 20.0);
        assert!(outcome.consume);
        // The outcome has no max-health field: a miniature heart cannot grant
        // permanent hearts (upstream only heals in `ItemHeart.finishUsingItem`).
    }

    #[test]
    fn a_creative_player_keeps_the_heart() {
        let c = config();
        let outcome = use_heart(MiniHeart::Blue, 5.0, 60.0, PlayerMode::Creative, &c)
            .expect("hearts are edible by default");
        assert_eq!(outcome.health, 60.0);
        assert!(!outcome.consume);
    }

    #[test]
    fn a_survival_player_consumes_the_heart_at_full_health_like_upstream() {
        let c = config();
        let outcome = use_heart(MiniHeart::Yellow, 20.0, 20.0, PlayerMode::Other, &c)
            .expect("hearts are edible by default");
        assert_eq!(outcome.health, 20.0);
        assert!(outcome.consume);
    }

    #[test]
    fn heart_eating_off_makes_a_heart_inert() {
        let c = parse_config("heart_eating = false\n");
        assert_eq!(
            use_heart(MiniHeart::Red, 2.0, 20.0, PlayerMode::Other, &c),
            None
        );
    }

    #[test]
    fn player_mode_only_creative_keeps_used_items() {
        assert_eq!(PlayerMode::of(1), PlayerMode::Creative);
        assert!(!PlayerMode::of(1).consumes());
        for mode in [0, 2, 3] {
            assert_eq!(PlayerMode::of(mode), PlayerMode::Other);
            assert!(PlayerMode::of(mode).consumes());
        }
    }

    #[test]
    fn a_heart_use_lasts_thirty_ticks() {
        assert_eq!(heart_use_duration(MiniHeart::Red, &config()), Some(30));
    }

    #[test]
    fn heart_eating_off_starts_no_use() {
        let c = parse_config("heart_eating = false\n");
        assert_eq!(heart_use_duration(MiniHeart::Red, &c), None);
    }
}
