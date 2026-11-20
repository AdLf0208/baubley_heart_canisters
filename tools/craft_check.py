#!/usr/bin/env python3
"""Craft the recipes Baubley Heart Canisters actually carries, in a real table.

Called by `tools/e2e.sh` with the open game's control socket. Uses the
harness's crafting grid (`ctl screen`: slots 100-108 row-major, result 200),
gives the ingredients and asserts, in this order:

1. upstream's empty `canister` (`data/bhc/recipe/canister.json`: 4 iron ingots
   around 1 bone, `#c:ingots/iron` + `#c:bones`) -> exactly one `bhc:canister`;
   then the result is shift-clicked into the inventory and the counts are read
   back, so the assertion covers the output id, its count and that the grid was
   consumed (4 iron and 1 bone gone, 1 canister gained);
2. the same ingredients in a pattern the recipe does not declare -> no result
   (the control: the grid is live, so an empty result is the recipe's answer);
3. a miniature heart has no carried recipe: 8 iron around 1 `bhc:red_heart`
   leaves the result empty. Upstream's `red_heart.json` needs eight
   `bhc:red_heart_melted` and a `c:hearts` member, and the melted-heart chain
   is deferred (notes.md, gap 5), so a heart in a table must craft nothing. If
   a later slice carries the chain this case changes with it.

The table is replaced before every case, so an untaken result cannot leak into
the next grid. Exits 1 with the observed result on the first mismatch.
"""
import json
import subprocess
import sys

# `canister.json`'s pattern, row-major: " I " / "I#I" / " I ".
IRON_SLOTS = ["101", "103", "105", "107"]
BONE_SLOT = ["104"]
WRONG_IRON_SLOTS = ["100", "102", "103", "105"]
GRID = ["100", "101", "102", "103", "104", "105", "106", "107", "108"]


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
        "8 iron around a miniature heart (no carried heart recipe)",
        [("minecraft:iron_ingot", 8), ("bhc:red_heart", 1)],
        [
            ("minecraft:iron_ingot", [s for s in GRID if s != "104"]),
            ("bhc:red_heart", ["104"]),
        ],
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
        "pattern craft nothing, and a miniature heart has no carried recipe"
    )


if __name__ == "__main__":
    main()
