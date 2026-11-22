#!/usr/bin/env python3
"""Craft the recipes Baubley Heart Canisters actually carries, in a real table.

Called by `tools/e2e.sh` with the open game's control socket. Uses the
harness's crafting grid (`ctl screen`: slots 100-108 row-major, result 200),
gives the ingredients and asserts, in this order, the result's **id and count**
and that the grid was **consumed**:

1. upstream's empty `canister` (`canister.json`: 4 iron ingots around 1 bone,
   `#c:ingots/iron` + `#c:bones`) -> exactly one `bhc:canister`;
2. the same ingredients in a pattern the recipe does not declare -> no result
   (the control: the grid is live, so an empty result is the recipe's answer);
3. 8 iron around a miniature heart -> no result, because the heart recipe needs
   the *melted* hearts, not iron;
4. the four heart recipes (`red_heart.json` ... `blue_heart.json`): eight of the
   colour's `bhc:<colour>_heart_melted` around **any** heart (`#c:hearts`, the
   tag the jar ships as `data/c/tags/item/hearts.json`) -> one miniature heart;
   each case puts a *different* colour in the middle, so the tag is what
   matched, not the result's own id;
5. the negative the recipe declares: seven melted hearts and a heart -> nothing
   (the pattern needs all eight);
6. the six tier-conversion recipes: nine lower melted hearts -> one higher
   (shaped) and one higher -> nine lower (shapeless), for red/yellow,
   yellow/green and green/blue.

The four smelting recipes are in a furnace, driven by `tools/e2e.sh` (the
100/200/300/400-tick cook times do not fit a grid probe). The table is replaced
before every case, so an untaken result cannot leak into the next grid. Exits 1
with the observed result on the first mismatch.
"""
import json
import subprocess
import sys

# `canister.json`'s pattern, row-major: " I " / "I#I" / " I ".
IRON_SLOTS = ["101", "103", "105", "107"]
BONE_SLOT = ["104"]
WRONG_IRON_SLOTS = ["100", "102", "103", "105"]
GRID = ["100", "101", "102", "103", "104", "105", "106", "107", "108"]
RING = [slot for slot in GRID if slot != "104"]
CENTRE = ["104"]

COLOURS = ["red", "yellow", "green", "blue"]
# The heart each heart recipe puts in the middle: deliberately not the colour it
# makes, so the `c:hearts` tag is what matched.
CENTRE_HEART = {"red": "blue", "yellow": "red", "green": "blue", "blue": "yellow"}


def chain_cases():
    """`red_heart.json` ... `blue_heart.json`: 8 melted + any heart -> 1 heart."""
    cases = []
    for colour in COLOURS:
        melted = f"bhc:{colour}_heart_melted"
        heart = f"bhc:{CENTRE_HEART[colour]}_heart"
        cases.append(
            {
                "label": f"{colour}_heart.json: 8 {melted} + 1 {heart} (#c:hearts)",
                "items": [(melted, 8), (heart, 1)],
                "place": [(melted, RING), (heart, CENTRE)],
                "want": (f"bhc:{colour}_heart", 1),
                "consume": [melted, heart],
            }
        )
    return cases


def conversion_cases():
    """The six tier recipes: nine up (shaped), one down (shapeless, -> 9)."""
    cases = []
    for low, high in zip(COLOURS, COLOURS[1:]):
        cases.append(
            {
                "label": f"{high}_heart_melted_from_{low}_heart_melted.json: 9 {low} -> 1 {high}",
                "items": [(f"bhc:{low}_heart_melted", 9)],
                "place": [(f"bhc:{low}_heart_melted", GRID)],
                "want": (f"bhc:{high}_heart_melted", 1),
                "consume": [f"bhc:{low}_heart_melted"],
            }
        )
        cases.append(
            {
                "label": f"{low}_heart_melted_from_{high}_heart_melted.json: 1 {high} -> 9 {low}",
                "items": [(f"bhc:{high}_heart_melted", 1)],
                "place": [(f"bhc:{high}_heart_melted", CENTRE)],
                "want": (f"bhc:{low}_heart_melted", 9),
                "consume": [f"bhc:{high}_heart_melted"],
            }
        )
    return cases


def main():
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

    def run(*args):
        # A bounded reply: a `ctl` call with no timeout blocks forever, and the
        # kill then looks like a pass (ports/_shared/launcher-e2e.md, trap 8).
        out = subprocess.run(
            [ctl, "--socket", sock, "--timeout", "30", *args],
            text=True,
            capture_output=True,
        )
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} in the inventory")

    def result():
        """The result slot's stack, or None: an empty slot has `item: null`."""
        stack = [s for s in slots() if s["slot"] == 200]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    def place(item, targets):
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            run("click-slot", str(target), "right")
            wait(1)
        run("click-slot", str(slot))
        wait(2)

    def setup(items):
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        # Replace the table so its block entity, and therefore the crafting
        # grid, is fresh for this case.
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        # The caller's teleport can be swallowed while the inventory screen is
        # still closing; a server `/tp` moves the player even then.
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(3)
        run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        reply = run("interact", x, y, z)
        print("debug interact:", reply)
        wait(5)
        # A menu is a client screen, so it can take a tick to appear; poll the
        # screen rather than assuming the first reply has slots.
        screen = {}
        for _ in range(10):
            screen = run("screen")
            if "slots" in screen:
                return
            wait(2)
        raise SystemExit(f"the crafting table did not open a menu: {screen}")

    def counts():
        return run("inventory").get("counts", {})

    failures = []

    def expect(label, items, placements, want_item, want_count, consume=None):
        setup(items)
        for item, targets in placements:
            place(item, targets)
        got = result()
        print(f"{label} ->", json.dumps(got))
        if not got or got.get("item") != want_item or got.get("count") != want_count:
            failures.append(f"{label}: expected {want_count} {want_item}, got {got}")
            return
        run("click-slot", "200", "shift")
        wait(3)
        run("close")
        wait(3)
        got_counts = counts()
        print("  inventory after taking it ->", json.dumps(got_counts))
        if got_counts.get(want_item, 0) != want_count:
            failures.append(
                f"{label}: the inventory holds {got_counts.get(want_item, 0)} {want_item}, "
                f"expected {want_count}"
            )
        for item in consume or []:
            if got_counts.get(item, 0) != 0:
                failures.append(
                    f"{label}: {got_counts.get(item, 0)} {item} left, so the grid was not consumed"
                )

    def expect_empty(label, items, placements):
        setup(items)
        for item, targets in placements:
            place(item, targets)
        got = result()
        print(f"{label} ->", json.dumps(got))
        if got is not None:
            failures.append(f"{label}: control crafted {got.get('count')} {got.get('item')}")

    expect(
        "4 iron + 1 bone (canister.json)",
        [("minecraft:iron_ingot", 4), ("minecraft:bone", 1)],
        [("minecraft:iron_ingot", IRON_SLOTS), ("minecraft:bone", BONE_SLOT)],
        "bhc:canister",
        1,
        consume=["minecraft:iron_ingot", "minecraft:bone"],
    )
    expect_empty(
        "the same ingredients in a pattern the recipe does not declare",
        [("minecraft:iron_ingot", 4), ("minecraft:bone", 1)],
        [("minecraft:iron_ingot", WRONG_IRON_SLOTS), ("minecraft:bone", BONE_SLOT)],
    )
    expect_empty(
        "8 iron around a miniature heart (the recipe needs melted hearts)",
        [("minecraft:iron_ingot", 8), ("bhc:red_heart", 1)],
        [("minecraft:iron_ingot", RING), ("bhc:red_heart", CENTRE)],
    )

    for case in chain_cases():
        expect(
            case["label"],
            case["items"],
            case["place"],
            case["want"][0],
            case["want"][1],
            consume=case["consume"],
        )

    expect_empty(
        "7 melted hearts + 1 heart (the recipe needs all eight)",
        [("bhc:red_heart_melted", 7), ("bhc:red_heart", 1)],
        [("bhc:red_heart_melted", RING[:7]), ("bhc:red_heart", CENTRE)],
    )

    for case in conversion_cases():
        expect(
            case["label"],
            case["items"],
            case["place"],
            case["want"][0],
            case["want"][1],
            consume=case["consume"],
        )

    # Leave one crafted canister in the inventory: every case above starts with
    # `clear @s`, and the caller asserts that the *crafted* item raises max
    # health, so the last grid must be the real recipe again.
    expect(
        "4 iron + 1 bone again, left in the inventory",
        [("minecraft:iron_ingot", 4), ("minecraft:bone", 1)],
        [("minecraft:iron_ingot", IRON_SLOTS), ("minecraft:bone", BONE_SLOT)],
        "bhc:canister",
        1,
        consume=["minecraft:iron_ingot", "minecraft:bone"],
    )

    run("close")
    wait(3)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(
        "recipe PASS: the carried canister recipe crafts exactly one bhc:canister "
        "from 4 iron + 1 bone and consumes them, the same ingredients in another "
        "pattern craft nothing, 8 iron around a heart craft nothing, all four heart "
        "recipes craft one heart of their colour from eight of their melted hearts "
        "plus any #c:hearts heart (and seven melteds craft nothing), and all six "
        "tier recipes trade nine lower melted hearts for one higher and one higher "
        "for nine lower — every case consuming its grid"
    )


if __name__ == "__main__":
    main()
