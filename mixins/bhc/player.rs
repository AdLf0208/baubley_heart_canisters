// End of a player's server tick (API 31): count carried canisters and apply
// the max-health modifier.
fn observe_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::bhc_state::on_player_tick(game);
}
