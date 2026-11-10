// Runs at the head of `Game::native_loot_drops` (API 75): the mod's say on
// the drops being rolled. The support module filters to mob deaths and adds
// the configured miniature hearts.
fn observe_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    crate::game::bhc_state::on_loot_drops(game, context, drops);
}
