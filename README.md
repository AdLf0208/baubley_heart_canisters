# bhc (Horizon port)

A Horizon port of [bhc](Traverse-Joe/Baubley-Heart-Canisters) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- The four miniature hearts are **consumable heals, not permanent hearts**.
- `HeartType` gives each tier a `healAmount` (`HeartType.java:3-24`: red 20,
- yellow 40, green 60, blue 80; soul 0) and `ItemHeart` uses `UseAnim.EAT` with
- `getUseDuration` 30 ticks (`ItemHeart.java:38-52`). `finishUsingItem` calls
- `entityLiving.heal(this.type.healAmount)`, shrinks the stack by one unless the
- player is creative, and calls `ServerPlayer.resetSentInfo()` to force the
- health sync (`ItemHeart.java:54-69`). It never touches
- `Attributes.MAX_HEALTH`: the mod's only permanent-heart behaviour is the

## Not yet ported

- Inventory scan instead of Curios.** Upstream grants the bonus only while a
- Heart Amulet is equipped (`ItemHeartAmulet.java:61-81`). With Curios and the
- amulet deferred, the port's main-inventory/off-hand scan stands in for the
- amulet's colour slots. A player can therefore carry all five colours at once
- (the soul amulet's five slots) rather than needing an amulet.
- Soul canisters count.** `getHeartCount` sums every stored canister,
- including the soul slot, so soul contributes 2 health each. `HeartType`'s
- `SOUL(0,0)` values describe the edible soul heart/belt, not the canister.

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: Traverse-Joe/Baubley-Heart-Canisters. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
