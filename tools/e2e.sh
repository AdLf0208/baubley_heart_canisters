#!/usr/bin/env bash
# In-game check for Baubley Heart Canisters slices 1 to 3.
#
# Upstream's formula (`HealthModifier.java:161`) is 2 health points per stored
# canister, capped per colour by `heartStackSize` (default 10). Slice 1 counts
# the canisters carried in the main inventory instead of a Curios amulet.
#
# Default run: baseline 20, one red canister -> 22, a second -> 24, a blue one
# stacks -> 26, removing the blue returns to 24, 12 red are capped at 10 ->
# 40, clearing red returns to 20, two red -> 24, and 24 survives a death
# (keepInventory) and a full relaunch. A stack of sticks changes nothing.
#
# Slice 2: with `mob_drops = true` and every chance at 1.0, a zombie drops two
# red miniature hearts (the configured amount), a cow (passive, no configured
# group) drops none, and the canister health table above stays green.
#
# Slice 3: eating a miniature heart restores upstream's `HeartType.healAmount`
# (red 20, yellow 40, green 60, blue 80, `HeartType.java:4-8`) clamped to max
# health, consumes one unless the player is creative (`ItemHeart.java:55-68`),
# and never raises max health. The run raises max to 40 with ten more red
# canisters, damages to 16, eats a red heart (16 -> 36, exactly +20), checks
# the stack shrank, tests a stick as the no-heal control, and checks a
# creative use keeps the heart and a full-health survival use still consumes
# it (upstream wastes it).
#
# MODE=control writes `heart_stack_size = 0`, `mob_drops = false` and
# `heart_eating = false` before the launch: the same canisters leave max health
# at 20, the same zombie drops no hearts, and the same red heart neither heals
# nor is consumed. That is the differential: an implementation that ignored
# any of those config keys would reach 22/24, drop a heart, or heal in this
# run and fail.
#
# Default run (EMBEDDED=0): the game loads `content/` as a dev mod.
# EMBEDDED=1: the game is launched with **only the native bundle** (no
# `--dev-mod`) and the content comes from the bundle's embedded copy
# (`mixins/bhc/data/bhc-content.hmod`, the AE2 pattern). The run also checks
# the file the helper wrote. CONTROL=1 (default with EMBEDDED) first launches
# the same binary with `.horizon-cache` unusable, where the canister and heart
# ids must be unknown.
set -euo pipefail

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/bhc
# The control binary is shared at the campaign pin; the game is this port's build.
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/bhc-e2e.sock}
MODE=${MODE:-default}
# SELFTEST=1 keeps the default setup but asserts the first canister adds nothing,
# so the run must fail: the counterfactual proving the check can fail.
SELFTEST=${SELFTEST:-0}
EMBEDDED=${EMBEDDED:-0}
CONTROL=${CONTROL:-$EMBEDDED}
WORLD=BHCE2E
CONTROL_WORLD=BHCControl
RED=bhc:red_heart_canister
BLUE=bhc:blue_heart_canister

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$PORT/content/horizon-mod.toml" ] || { echo "missing content mod: $PORT/content" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/bhc-home.XXXXXX)
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

launch() {
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    local args=(--game "$GAME" --world "$world" --seed 20261104 --creative --headless --wait
                --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" >"$SCRATCH/game-console.log" 2>&1
    ctl wait-ready 120
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

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. The canister item must then be unknown; if it resolves, the run
# proves nothing about the embedded file.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    ctl command "item replace entity @s hotbar.0 with $RED 1" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "bhc:red_heart_canister" or .item == "bhc:red_heart")] | length == 0' >/dev/null \
        || fail "the control game has a BHC item, so the content did not come from the embedded file"
    ctl command "execute in minecraft:the_end run summon minecraft:item 0.5 64 0.5 {Item:{id:\"bhc:red_heart\",count:1}}" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    [ "$(hearts)" -eq 0 ] || fail "the control game spawned a bhc:red_heart entity, so the id was known"
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no canister item, no heart entity, .horizon-cache stayed a file"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/bhc-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/bhc.hmod" \
        || fail "the embedded content differs from content/bhc.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/bhc.hmod"
}

# The unit tests cover upstream's chances (red 0.15, the others 1.0) and the
# amount default of 1. The in-game run makes every chance 100 % and asks for
# two hearts so a drop is deterministic and its count is visible.
if [ "$MODE" = control ]; then
    cat > "$SCRATCH/config/bhc.conf" <<'EOF'
heart_stack_size = 0
mob_drops = false
heart_drop_amount = 2
red_heart_drop_chance = 1.0
yellow_heart_drop_chance = 1.0
green_heart_drop_chance = 1.0
blue_heart_drop_chance = 1.0
heart_eating = false
red_heart_heal = 20
yellow_heart_heal = 40
green_heart_heal = 60
blue_heart_heal = 80
EOF
    echo "control: heart_stack_size=0, mob_drops=false, heart_eating=false, chance=1.0, amount=2"
else
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
fi

echo "== launch (creative, The End arena, $([ "$EMBEDDED" = 1 ] && echo 'native bundle only (embedded content)' || echo 'content/dev-mod')) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
expect_health 20 "baseline"

echo "== mob drops: a configured zombie drops the configured red hearts =="
clear_items
ctl command "summon minecraft:zombie 2 64 2" >/dev/null || fail "cannot summon the zombie"
ctl wait 3
ctl command "kill @e[type=minecraft:zombie]" >/dev/null || fail "cannot kill the zombie"
ctl wait 3
red_dropped=$(ground_item bhc:red_heart)
if [ "$MODE" = control ]; then
    [ "$red_dropped" -eq 0 ] || fail "control: mob_drops=false still dropped $red_dropped red hearts"
    echo "control: the same zombie dropped $red_dropped red hearts"
else
    [ "$red_dropped" -eq 2 ] || fail "$red_dropped red hearts dropped, expected the configured 2"
    echo "zombie dropped $red_dropped red hearts (configured amount 2)"
    [ "$(hearts)" -eq 2 ] || fail "unexpected extra heart drops: $(hearts)"
fi

echo "== mob drops: a non-configured cow drops no hearts =="
clear_items
ctl command "summon minecraft:cow 2 64 2" >/dev/null || fail "cannot summon the cow"
ctl wait 3
ctl command "kill @e[type=minecraft:cow]" >/dev/null || fail "cannot kill the cow"
ctl wait 3
cow_hearts=$(hearts)
[ "$cow_hearts" -eq 0 ] || fail "the passive cow dropped $cow_hearts hearts"
echo "cow dropped $cow_hearts hearts (no configured group)"
clear_items

echo "== every content item id is giveable =="
ITEMS="bhc:canister bhc:red_heart bhc:yellow_heart bhc:green_heart bhc:blue_heart bhc:red_heart_canister bhc:yellow_heart_canister bhc:green_heart_canister bhc:blue_heart_canister bhc:soul_heart_canister"
for item in $ITEMS; do
    ctl give "$item" 1 >/dev/null || fail "the content mod did not provide $item"
done
ctl wait 8
for item in $ITEMS; do
    n=$(ctl inventory | jq -r --arg i "$item" '[.counts[$i] // 0] | add')
    [ "$n" -ge 1 ] || fail "$item was not in the inventory after /give"
    echo "giveable: $item"
done
ctl chat "/clear @s" >/dev/null
ctl wait 8
expect_health 20 "all items cleared again"

echo "== a stack of sticks is a control item =="
ctl give minecraft:stick 64 >/dev/null || fail "cannot give sticks"
ctl wait 6
expect_health 20 "64 sticks change nothing"
clear_item minecraft:stick

echo "== one red canister raises max health by 2 =="
ctl give "$RED" 1 >/dev/null || fail "the content mod did not provide $RED"
ctl wait 6
if [ "$MODE" = control ]; then
    expect_health 20 "control: one red canister is off"
elif [ "$SELFTEST" = 1 ]; then
    expect_health 20 "selftest: the canister's +1 heart is asserted absent (must FAIL)"
else
    expect_health 22 "one red canister (+1 heart)"
fi

echo "== a second red canister stacks to +4 =="
ctl give "$RED" 1 >/dev/null
ctl wait 6
if [ "$MODE" = control ]; then
    expect_health 20 "control: two red canisters are off"
else
    expect_health 24 "two red canisters (+2 hearts)"
fi

echo "== a blue canister adds its own colour =="
ctl give "$BLUE" 1 >/dev/null || fail "the content mod did not provide $BLUE"
ctl wait 6
if [ "$MODE" = control ]; then
    expect_health 20 "control: a blue canister is off"
else
    expect_health 26 "two red + one blue (+3 hearts)"
fi

echo "== removing the blue canister returns the bonus =="
clear_item "$BLUE"
if [ "$MODE" = control ]; then
    expect_health 20 "control: removing blue keeps 20"
else
    expect_health 24 "blue removed (+2 hearts left)"
fi

if [ "$MODE" = control ]; then
    echo "== control: heart_eating=false makes a miniature heart inert =="
    ctl command "gamerule natural_health_regeneration false" >/dev/null
    ctl command "gamerule spawn_mobs false" >/dev/null
    ctl command "difficulty peaceful" >/dev/null
    ctl command "kill @e[type=!player]" >/dev/null 2>&1 || true
    ctl command "kill @e[type=minecraft:enderman]" >/dev/null 2>&1 || true
    ctl command "gamemode survival" >/dev/null
    ctl wait 5
    heal_full
    expect_current_health 20 "control: survival baseline"
    hit
    hit
    hit
    expect_current_health 2 "control: damaged to 2"
    ctl give bhc:red_heart 1 >/dev/null || fail "cannot give the control heart"
    ctl wait 4
    select_item bhc:red_heart
    red_before=$(item_count bhc:red_heart)
    use_held
    expect_current_health 2 "control: a heart heals nothing"
    red_after=$(item_count bhc:red_heart)
    [ "$red_after" -eq "$red_before" ] \
        || fail "control: heart_eating=false still consumed a heart ($red_before -> $red_after)"
    echo "control: heart count stayed $red_after"

    ctl quit >/dev/null 2>&1 || true
    echo "CONTROL PASS: canisters stayed at 20 max health with heart_stack_size=0, the zombie dropped no hearts with mob_drops=false, and a red heart neither healed nor was consumed with heart_eating=false"
    exit 0
fi

echo "== twelve red count as ten: the per-colour cap =="
ctl give "$RED" 10 >/dev/null
ctl wait 6
expect_health 40 "12 red capped at 10 canisters (+20 hearts)"

echo "== removing them all returns max health to 20 =="
clear_item "$RED"
expect_health 20 "all canisters removed"

echo "== two red again, then a death and respawn =="
ctl give "$RED" 2 >/dev/null
ctl wait 6
expect_health 24 "two red again"
ctl command "gamerule keep_inventory true" >/dev/null
ctl command "kill @s" >/dev/null || true
ctl wait 25
ctl respawn >/dev/null || fail "cannot respawn"
ctl wait 25
expect_health 24 "bonus re-applied after respawn"

echo "== quit and relaunch the same world =="
ctl quit >/dev/null 2>&1 || true
sleep 2
launch
expect_health 24 "bonus re-applied after relaunch"

echo "== miniature hearts: a creative use keeps the heart =="
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

echo "== miniature hearts: 12 red canisters raise max to 40 =="
ctl command "gamemode survival" >/dev/null
ctl wait 4
ctl give "$RED" 10 >/dev/null || fail "cannot give the canisters"
ctl wait 6
expect_health 40 "12 red capped at 10 (+20 hearts)"
expect_current_health 40 "canisters filled the new health"

echo "== a red heart heals exactly 20 and is consumed =="
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

echo "== a stick is the no-heal control =="
hit
hit
expect_current_health 24 "damaged to 24 again"
ctl give minecraft:stick 1 >/dev/null || fail "cannot give the stick"
ctl wait 4
select_item minecraft:stick
use_held
expect_current_health 24 "a stick heals nothing"

echo "== a full-health survival use still consumes a heart, as upstream does =="
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

echo "PASS: a zombie dropped 2 configured red hearts and a cow dropped none, 1 red 22, 2 red 24, +blue 26, -blue 24, 12 red capped at 40, clear 20, 24 survived respawn + relaunch, a red heart healed exactly 20 (16 -> 36) and was consumed, a stick healed nothing, creative kept its heart, and a full-health survival use still consumed one; max health never rose from eating"
