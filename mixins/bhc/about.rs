// Before the server starts: load or create `config/bhc.conf`.
fn load_config(game: &mut crate::game::Game<'_>) {
    crate::game::bhc_state::on_server_about_to_start(game);
}
