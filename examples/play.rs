use std::env;

use librespot::core::authentication::Credentials;
use librespot::core::config::SessionConfig;
use librespot::core::session::Session;
use librespot::core::spotify_id::SpotifyId;
use librespot::playback::config::PlayerConfig;
use librespot::playback::audio_backend;
use librespot::playback::player::Player;

#[tokio::main]
async fn main() {
    let session_config = SessionConfig::default();
    let player_config = PlayerConfig::default();

    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} USERNAME PASSWORD TRACK", args[0]);
        return;
    }
    let credentials = Credentials::with_password(&args[1], &args[2]);

    let track = SpotifyId::from_base62("1hHuyqVCZCbhYQixEkdQCo").unwrap();

    let backend = audio_backend::find(None).unwrap();

    println!("Connecting ..");
    let session = Session::connect(session_config, credentials, None).await.unwrap();

    let (mut player, _) = Player::new(player_config, session, None, move || {
        backend(None)
    });

    player.load(track, true, 0);

    println!("Playing...");

    player.await_end_of_track().await;

    println!("Done");
}
