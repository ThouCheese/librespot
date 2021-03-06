use env_logger;
use std::env;

use librespot::core::authentication::Credentials;
use librespot::core::config::SessionConfig;
use librespot::core::session::Session;
use librespot::core::spotify_id::SpotifyId;
use librespot::metadata::{Metadata, Playlist, Track};

#[tokio::main]
async fn main() {
    env_logger::init();
    let session_config = SessionConfig {
        user_agent: "just testing".to_string(),
        device_id: "testing device".to_string(),
        proxy: None,
        ap_port: None,
    };

    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: {} USERNAME PASSWORD PLAYLIST", args[0]);
    }
    let username = args[1].to_owned();
    let password = args[2].to_owned();
    let credentials = Credentials::with_password(username, password);

    let uri_split = args[3].split(":");
    let uri_parts: Vec<&str> = uri_split.collect();
    println!("{}, {}, {}", uri_parts[0], uri_parts[1], uri_parts[2]);

    let plist_uri = SpotifyId::from_base62(uri_parts[2]).unwrap();

    let session = Session::connect(session_config, credentials, None).await.unwrap();

    let plist = Playlist::get(&session, plist_uri).await.unwrap();
    println!("{:?}", plist);
    for track_id in plist.tracks {
        let plist_track = Track::get(&session, track_id).await.unwrap();
        println!("track: {} ", plist_track.name);
    }
}
