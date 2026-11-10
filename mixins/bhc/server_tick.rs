// At the end of a running server tick: resend the guest inventory after a
// heart use decremented the acting hand. The hub's per-tick slot diff is
// refreshed during the guest turn, so it misses an in-use decrement.
fn observe_server_tick(game: &mut crate::game::Game<'_>, phase: ServerTickPhase) {
    crate::game::bhc_state::on_server_tick(game, phase);
}
