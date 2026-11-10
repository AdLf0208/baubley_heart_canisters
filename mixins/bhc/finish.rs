// Runs at the head of `Game::native_living_entity_use_item_finish` (API 90):
// the 30-tick heart use completed; heal and leave the remainder in `result`.
fn observe_heart_finish(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    entity: Option<i32>,
    hand: crate::first_person::Hand,
    duration: i32,
    result: &mut Option<ItemStack>,
) {
    let _ = duration;
    crate::game::bhc_state::on_use_item_finish(game, stack, entity, hand, result);
}
