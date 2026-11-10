// Runs at the head of `Game::native_player_respawned` (API 21): the respawned
// body gets the canisters' max-health modifier again.
fn observe_respawned(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    crate::game::bhc_state::on_player_respawned(game, player, name);
}
