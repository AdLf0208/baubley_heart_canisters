// Runs at the head of `Game::native_use_duration` (API 90): a miniature heart
// is eaten over upstream's 30 ticks when `heart_eating` is on.
fn observe_use_duration(stack: &ItemStack, duration: &mut i32) {
    crate::game::bhc_state::on_use_duration(stack, duration);
}
