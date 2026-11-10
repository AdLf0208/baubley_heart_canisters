// Runs at the head of `Game::native_use_animation` (API 90): a miniature heart
// uses the vanilla eat pose while it is chewed.
fn observe_use_animation(
    stack: &ItemStack,
    animation: &mut horizon_world::consumable::UseAnimation,
) {
    crate::game::bhc_state::on_use_animation(stack, animation);
}
