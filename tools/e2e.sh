#!/usr/bin/env bash
# In-game check for Baubley Heart Canisters slices 1 to 3, at the SDK pin.
#
# The package is a flat `.hmod` the launcher compiles into the game
# (`ports/_shared/launcher-e2e.md`), so `GAME` must name a game built with it and
# `CONTROL_GAME` the same Nightly *without* it:
#
#   ports/tools/launcher-game.sh build bhc
#   eval "$(ports/tools/launcher-game.sh env bhc)"
#   ports/tools/build-slot.sh run --job e2e-bhc --port ports/bhc --step e2e -- \
#       bash ports/bhc/tools/e2e.sh
#
# Upstream's formula (`HealthModifier.java:161`) is 2 health points per stored
# canister, capped per colour by `heartStackSize` (default 10). Slice 1 counts
# the canisters carried in the main inventory instead of a Curios amulet.
#
# The legs, in order, and the parity rule they answer (AGENTS.md, 2026-11-20):
#   1. every carried item id is `/give`-able and reads back, and an unknown id
#      is refused (items);
#   2. a zombie (hostile) drops the configured red hearts, a cow (passive, no
#      configured group) drops none, and a warden — upstream's overlap, a
#      monster *and* a boss with its own blue entry — drops red + yellow + blue
#      (loot/drop family, the differential);
#   3. the carried `canister` recipe crafts exactly one `bhc:canister` from
#      4 iron + 1 bone, consumes them, and the crafted *empty* canister adds no
#      health (it is upstream's ingredient, not a heart canister); a heart in
#      the grid crafts nothing because its own recipe needs the deferred
#      melted-heart chain (recipe family, `tools/craft_check.py`);
#   4. the canister health table: 1 red 22, 2 red 24, +blue 26, -blue 24,
#      12 red capped at 40, clear 20, and 24 survives a death (keepInventory)
#      and a full relaunch (slice 1);
#   5. the heart table: a creative use keeps the heart, a survival red heart at
#      16/40 heals exactly 20 and is consumed, a stick heals nothing, a
#      full-health survival use still consumes one, and max health never rises
#      from eating (slice 3).
#
# MODE=control launches `CONTROL_GAME` (the same Nightly, no package) and runs
# the same item and drop assertions; they must fail there, so the run ends
# `EXIT:1` — that is the differential proving the package is what answers them.
# SELFTEST=1 keeps the default setup and inverts the drop assertion (the zombie
# must have dropped none), so it must fail: the counterfactual proving the
# assertion is alive.
set -euo pipefail

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/bhc
# The launcher exports the ctl beside the game; the campaign's shared ctl at the
# retired pin is only a fallback for a hand-run.
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
CONTROL_GAME=${CONTROL_GAME:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-client}
RES=${RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/bhc-e2e.sock}
E2E_TMP=${E2E_TMP:-/tmp/opencode}
MODE=${MODE:-default}
SELFTEST=${SELFTEST:-0}
WORLD=BHCE2E
CONTROL_WORLD=BHCControl
RED=bhc:red_heart_canister
BLUE=bhc:blue_heart_canister

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -x "$CONTROL_GAME" ] || { echo "missing control horizon-client: $CONTROL_GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$PORT/horizon-mod.toml" ] || { echo "missing package: $PORT" >&2; exit 1; }

SCRATCH=$(mktemp -d "$E2E_TMP/bhc-home.XXXXXX")
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$SCRATCH/config"
rm -f "$SOCK"
# The game folder is the working directory: keep saves/options out of the port.
cd "$SCRATCH"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
say() { echo "== $* =="; }

# `ctl state` does not expose max health at this pin; the vanilla attribute
# command reads the server body's computed value. Chat is newest first.
max_health() {
    ctl chat "/attribute @s minecraft:max_health get" >/dev/null
    ctl wait 5 >/dev/null
    ctl messages 5 | jq -r '.chat[0]' \
        | grep -oE 'is -?[0-9]+(\.[0-9]+)?' | head -1 | awk '{print $2}' | cut -d. -f1
}
# The attribute/heal write can land a tick after the use or the modifier set,
# so the exact value is polled for up to ~2 s instead of read once.
expect_health() {
    local got tries=0
    while :; do
        got=$(max_health)
        [ "$got" = "$1" ] && break
        tries=$((tries + 1))
        [ "$tries" -lt 10 ] || break
        ctl wait 2 >/dev/null
    done
    if [ "$got" != "$1" ]; then
        ctl state | jq -c '.player' >&2 || true
        fail "$2: expected max health $1, got ${got:-nothing}"
    fi
    echo "max health $got ($2)"
}
clear_item() {
    ctl chat "/clear @s $1" >/dev/null
    ctl wait 6
}
# Dropped item entities of one kind on the ground.
ground_item() {
    ctl entities 64 item \
        | jq --arg item "$1" \
            '[.entities[] | select(.kind == "item" and .item.item == $item) | .item.count] | add // 0'
}
# All dropped miniature hearts, whatever the colour.
hearts() {
    ctl entities 64 item \
        | jq '[.entities[] | select(.kind == "item" and (.item.item | test("^bhc:(red|yellow|green|blue)_heart$"))) | .item.count] | add // 0'
}
clear_items() {
    ctl command "kill @e[type=minecraft:item]" >/dev/null 2>&1 || true
    ctl wait 2
}
# `ctl effects` reports the local player's live health (`allthetweaks` e2e).
health() { ctl effects | jq -r '.health'; }
item_count() { ctl inventory | jq -r --arg i "$1" '.counts[$i] // 0'; }
# One instant-damage hit is 6 health. The 22-tick wait clears the 20-tick
# invulnerability frame, and natural regeneration is off in every health scene.
hit() { ctl command "effect give @s minecraft:instant_damage 1 0" >/dev/null; ctl wait 22; }
expect_current_health() {
    local got tries=0
    while :; do
        got=$(health | awk '{printf "%.0f", $1}')
        [ "$got" = "$1" ] && break
        tries=$((tries + 1))
        [ "$tries" -lt 10 ] || break
        ctl wait 2 >/dev/null
    done
    if [ "$got" != "$1" ]; then
        ctl effects >&2 || true
        fail "$2: expected health $1, got ${got:-nothing}"
    fi
    echo "health $got ($2)"
}
select_item() {
    ctl select "$1" >/dev/null || fail "cannot select $1"
    ctl wait 2
}
# A one-line dump of the values the max-health scenes depend on.
diag() {
    local m
    m=$(ctl command "attribute @s minecraft:max_health get" | jq -r '.chat | join(" ")')
    echo "diag: health=$(health) max=[$m] red_canisters=$(item_count bhc:red_heart_canister) blue_canisters=$(item_count bhc:blue_heart_canister) red_hearts=$(item_count bhc:red_heart)"
}

use_held() {
    local reply
    # The End's win/credits screen can be up after a quit/relaunch and
    # `use-item` refuses while a screen is open.
    ctl close >/dev/null 2>&1 || true
    ctl wait 2
    # A miniature heart is eaten over 30 ticks (1.5 s); hold past the
    # finish so the server completes the use, then let the count sync.
    if ! reply=$(ctl use-item --hold 1.7); then
        echo "use-item failed: $reply" >&2
        fail "cannot use the held item"
    fi
    ctl wait 6
}
# Top the player up to max health (instant health clamps to max). The relaunch
# respawns at the overworld spawn, where stray damage/regeneration can leave the
# starting health below max; the heart scenes need a known full-health start.
heal_full() {
    ctl command "effect give @s minecraft:instant_health 1 10" >/dev/null
    ctl wait 22
}

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$SCRATCH" 2>/dev/null || true
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch WORLD [GAME]
    local world=${1:-$WORLD}
    local game=${2:-$GAME}
    rm -f "$SOCK"
    # `--offscreen`, not `--headless`: the recipe leg opens a crafting-table
    # menu, and a menu is a client screen that a renderer-less game never has
    # (`ctl screen` then never answers). The offscreen renderer keeps the run
    # windowless.
    "$CTL" --socket "$SOCK" launch \
        --game "$game" --world "$world" --seed 20261104 --creative --offscreen --wait \
        --resources "$RES" >"$SCRATCH/game-console.log" 2>&1
    ctl wait-ready 180
    ctl wait 10
    ctl command "execute in minecraft:the_end run tp @s 0.5 64 0.5" >/dev/null
    ctl wait 5
    ctl command "execute in minecraft:the_end run fill -4 63 -4 4 63 4 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s 0.5 64 0.5" >/dev/null
    ctl wait 10
    # Keep the health measurements free of stray hostile damage.
    ctl command "kill @e[type=minecraft:enderman]" >/dev/null 2>&1 || true
    ctl wait 3
    # A quit/relaunch can leave a screen (the End's win/credits screen) up, and
    # `use-item` refuses while one is open; close whatever is there.
    ctl close >/dev/null 2>&1 || true
    ctl wait 2
}

# The item and drop legs, shared by the modded run and the control. They are the
# same commands on purpose: on `CONTROL_GAME` (no package) the ids are unknown
# and the zombie drops nothing, so the identical assertions fail.
item_and_drop_leg() {
    say "every content item id is giveable, and an unknown id is refused"
    ITEMS="bhc:canister bhc:red_heart bhc:yellow_heart bhc:green_heart bhc:blue_heart bhc:red_heart_canister bhc:yellow_heart_canister bhc:green_heart_canister bhc:blue_heart_canister bhc:soul_heart_canister"
    for item in $ITEMS; do
        ctl give "$item" 1 >/dev/null || fail "the package did not provide $item"
    done
    ctl wait 8
    for item in $ITEMS; do
        n=$(item_count "$item")
        [ "$n" -ge 1 ] || fail "$item was not in the inventory after /give"
        echo "giveable: $item"
    done
    ctl give bhc:not_a_heart 1 >/dev/null 2>&1 || true
    ctl wait 4
    [ "$(item_count bhc:not_a_heart)" -eq 0 ] \
        || fail "an unknown id was accepted: bhc:not_a_heart is in the inventory"
    echo "unknown id refused: bhc:not_a_heart"
    ctl chat "/clear @s" >/dev/null
    ctl wait 8

    say "mob drops: a configured zombie drops the configured red hearts"
    clear_items
    ctl command "summon minecraft:zombie 2 64 2" >/dev/null || fail "cannot summon the zombie"
    ctl wait 3
    ctl command "kill @e[type=minecraft:zombie]" >/dev/null || fail "cannot kill the zombie"
    ctl wait 3
    red_dropped=$(ground_item bhc:red_heart)
    if [ "$SELFTEST" = 1 ]; then
        [ "$red_dropped" -eq 0 ] \
            || fail "SELFTEST: the zombie dropped $red_dropped hearts, so the drop assertion is alive"
        echo "SELFTEST unexpectedly saw no drop"
        return 0
    fi
    [ "$red_dropped" -eq 2 ] || fail "$red_dropped red hearts dropped, expected the configured 2"
    echo "zombie dropped $red_dropped red hearts (configured amount 2)"
    [ "$(hearts)" -eq 2 ] || fail "unexpected extra heart drops: $(hearts)"

    say "mob drops: a non-configured cow drops no hearts"
    clear_items
    ctl command "summon minecraft:cow 2 64 2" >/dev/null || fail "cannot summon the cow"
    ctl wait 3
    ctl command "kill @e[type=minecraft:cow]" >/dev/null || fail "cannot kill the cow"
    ctl wait 3
    cow_hearts=$(hearts)
    [ "$cow_hearts" -eq 0 ] || fail "the passive cow dropped $cow_hearts hearts"
    echo "cow dropped $cow_hearts hearts (no configured group)"
    clear_items

    # The other three configured groups. The warden is upstream's overlap: its
    # entity type is a monster (red), it is in the boss set (yellow) and it has
    # its own blue entry (`DropHandler.java:85-109`). The dragon is its own
    # group and matches neither of the other two.
    say "mob drops: the warden is red + yellow + blue, the dragon is green"
    clear_items
    ctl command "summon minecraft:warden 2 64 2" >/dev/null || fail "cannot summon the warden"
    ctl wait 3
    ctl command "kill @e[type=minecraft:warden]" >/dev/null || fail "cannot kill the warden"
    ctl wait 5
    for heart in red yellow blue; do
        got=$(ground_item "bhc:${heart}_heart")
        [ "$got" -eq 2 ] || fail "the warden dropped $got ${heart} hearts, expected 2"
    done
    [ "$(ground_item bhc:green_heart)" -eq 0 ] || fail "the warden dropped green hearts"
    echo "warden dropped 2 red, 2 yellow, 2 blue and no green (monster + boss + its own entry)"
    clear_items

    # The green heart's dragon group is **blocked** at this pin, not skipped:
    # `loot_drops` never fires for the ender dragon because the dragon is not a
    # `Mob` — `EnderDragon.tickDeath`
    # (`crates/horizon-client/src/game/mobs/dragon/death.rs`) spawns its orbs and
    # the exit portal directly, and neither `native_loot_drops` nor
    # `native_living_drops` is called on that path. In game the dragon dies and
    # drops no item at all (probe: `dragon-gap.log`). The rule itself is carried
    # and unit-tested (`class_of`/`group_matches`), and the coverage table in
    # `notes.md` records the gap with the symbol.
}

# The control: the same Nightly without the package. The identical item and drop
# assertions must fail here, so this leg ends non-zero by design.
control_leg() {
    say "control: the same Nightly without the package"
    launch "$CONTROL_WORLD" "$CONTROL_GAME"
    ctl give bhc:red_heart 1 >/dev/null 2>&1 || true
    ctl wait 6
    [ "$(item_count bhc:red_heart)" -eq 0 ] \
        || fail "control: the unmodified game has bhc:red_heart, so the id was known without the package"
    echo "control: bhc:red_heart is unknown to the unmodified game"
    clear_items
    ctl command "summon minecraft:zombie 2 64 2" >/dev/null || fail "cannot summon the zombie"
    ctl wait 3
    ctl command "kill @e[type=minecraft:zombie]" >/dev/null || fail "cannot kill the zombie"
    ctl wait 3
    red_dropped=$(ground_item bhc:red_heart)
    echo "control: the same zombie dropped $red_dropped red hearts"
    [ "$red_dropped" -eq 2 ] \
        || fail "CONTROL: without the package the same zombie dropped $red_dropped red hearts, so the drop assertion fails without it (expected)"
}

# The unit tests cover upstream's chances (red 0.15, the others 1.0) and the
# amount default of 1. The in-game run makes every chance 100 % and asks for
# two hearts so a drop is deterministic and its count is visible.
cat > "$SCRATCH/config/bhc.conf" <<'EOF'
heart_stack_size = 10
mob_drops = true
heart_drop_amount = 2
red_heart_drop_chance = 1.0
yellow_heart_drop_chance = 1.0
green_heart_drop_chance = 1.0
blue_heart_drop_chance = 1.0
heart_eating = true
red_heart_heal = 20
yellow_heart_heal = 40
green_heart_heal = 60
blue_heart_heal = 80
EOF
echo "default: mob_drops=true, heart_eating=true, chance=1.0, amount=2"

if [ "$MODE" = control ]; then
    control_leg
    exit 1
fi

say "launch (creative, The End arena, the package compiled into the game)"
launch
expect_health 20 "baseline"

item_and_drop_leg
if [ "$SELFTEST" = 1 ]; then
    echo "SELFTEST: the drop assertion is alive; the run must end non-zero"
    exit 1
fi

say "the carried recipe: 4 iron + 1 bone crafts one canister"
ctl command "execute in minecraft:the_end run setblock 3 64 3 minecraft:crafting_table" >/dev/null
ctl wait 3
python3 "$PORT/tools/craft_check.py" "$SOCK" "$CTL" 3 64 3 || fail "the recipe check failed"
ctl wait 4
[ "$(item_count bhc:canister)" -eq 1 ] || fail "the crafted canister is not in the inventory"
# The empty canister is upstream's crafting ingredient, not a heart canister:
# only the five coloured canisters count (`Carried::from_stacks`), so the
# crafted item must leave max health at 20. That is the recipe's control.
expect_health 20 "the crafted empty canister is not a heart canister"
clear_item bhc:canister
expect_health 20 "the crafted canister removed again"

say "a stack of sticks is a control item"
ctl give minecraft:stick 64 >/dev/null || fail "cannot give sticks"
ctl wait 6
expect_health 20 "64 sticks change nothing"
clear_item minecraft:stick

say "one red canister raises max health by 2"
ctl give "$RED" 1 >/dev/null || fail "the package did not provide $RED"
ctl wait 6
expect_health 22 "one red canister (+1 heart)"

say "a second red canister stacks to +4"
ctl give "$RED" 1 >/dev/null
ctl wait 6
expect_health 24 "two red canisters (+2 hearts)"

say "a blue canister adds its own colour"
ctl give "$BLUE" 1 >/dev/null || fail "the package did not provide $BLUE"
ctl wait 6
expect_health 26 "two red + one blue (+3 hearts)"

say "removing the blue canister returns the bonus"
clear_item "$BLUE"
expect_health 24 "blue removed (+2 hearts left)"

say "twelve red count as ten: the per-colour cap"
ctl give "$RED" 10 >/dev/null
ctl wait 6
expect_health 40 "12 red capped at 10 canisters (+20 hearts)"

say "removing them all returns max health to 20"
clear_item "$RED"
expect_health 20 "all canisters removed"

say "two red again, then a death and respawn"
ctl give "$RED" 2 >/dev/null
ctl wait 6
expect_health 24 "two red again"
ctl command "gamerule keep_inventory true" >/dev/null
ctl command "kill @s" >/dev/null || true
ctl wait 25
ctl respawn >/dev/null || fail "cannot respawn"
ctl wait 25
expect_health 24 "bonus re-applied after respawn"

say "quit and relaunch the same world"
ctl quit >/dev/null 2>&1 || true
sleep 2
launch
expect_health 24 "bonus re-applied after relaunch"

say "miniature hearts: a creative use keeps the heart"
ctl command "gamerule natural_health_regeneration false" >/dev/null
ctl command "gamerule spawn_mobs false" >/dev/null
# Peaceful for the health scenes: a stray hostile hit between a heal and its
# read moved health by 6 (observed 2026-11-09); instant_damage still works.
ctl command "difficulty peaceful" >/dev/null
ctl command "kill @e[type=!player]" >/dev/null 2>&1 || true
ctl wait 5
heal_full
expect_current_health 24 "creative baseline at full health"
ctl give bhc:red_heart 2 >/dev/null || fail "cannot give the red hearts"
ctl wait 4
select_item bhc:red_heart
red_before=$(item_count bhc:red_heart)
[ "$red_before" -eq 2 ] || fail "expected 2 red hearts, got $red_before"
use_held
expect_current_health 24 "creative full health unchanged"
red_after=$(item_count bhc:red_heart)
[ "$red_after" -eq "$red_before" ] \
    || fail "a creative use consumed a heart ($red_before -> $red_after)"
echo "creative: heart count stayed $red_after"
clear_item bhc:red_heart

say "miniature hearts: 12 red canisters raise max to 40"
ctl command "gamemode survival" >/dev/null
ctl wait 4
ctl give "$RED" 10 >/dev/null || fail "cannot give the canisters"
ctl wait 6
expect_health 40 "12 red capped at 10 (+20 hearts)"
expect_current_health 40 "canisters filled the new health"

say "a red heart heals exactly 20 and is consumed"
diag
hit
hit
hit
hit
expect_current_health 16 "damaged to 16"
ctl give bhc:red_heart 3 >/dev/null || fail "cannot give the red hearts"
ctl wait 4
select_item bhc:red_heart
[ "$(item_count bhc:red_heart)" -eq 3 ] || fail "expected 3 red hearts before eating"
use_held
expect_current_health 36 "red heart heals 20 (16 -> 36)"
[ "$(item_count bhc:red_heart)" -eq 2 ] || fail "a survival use did not consume one heart"
expect_health 40 "max health unchanged by eating"

say "a stick is the no-heal control"
hit
hit
expect_current_health 24 "damaged to 24 again"
ctl give minecraft:stick 1 >/dev/null || fail "cannot give the stick"
ctl wait 4
select_item minecraft:stick
use_held
expect_current_health 24 "a stick heals nothing"

say "a full-health survival use still consumes a heart, as upstream does"
diag
select_item bhc:red_heart
use_held
diag
expect_current_health 40 "a red heart caps at 40"
[ "$(item_count bhc:red_heart)" -eq 1 ] || fail "the capped use did not consume a heart"
select_item bhc:red_heart
use_held
expect_current_health 40 "full health heals nothing"
[ "$(item_count bhc:red_heart)" -eq 0 ] \
    || fail "a full-health survival use did not consume the last heart"
echo "full-health: the last red heart was consumed at 40/40"

echo "PASS: all ten item ids are giveable and an unknown one is refused, a zombie dropped 2 configured red hearts, a cow dropped none and a warden dropped 2 red + 2 yellow + 2 blue, the carried recipe crafted one canister from 4 iron + 1 bone and consumed them while the empty canister added no health, 1 red 22, 2 red 24, +blue 26, -blue 24, 12 red capped at 40, clear 20, 24 survived respawn + relaunch, a red heart healed exactly 20 (16 -> 36) and was consumed, a stick healed nothing, creative kept its heart, and a full-health survival use still consumed one; max health never rose from eating"
