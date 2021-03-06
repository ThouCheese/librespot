use std::env;

use librespot::core::authentication::Credentials;
use librespot::core::config::SessionConfig;
use librespot::core::keymaster;
use librespot::core::session::Session;

const SCOPES: &str =
    "streaming,user-read-playback-state,user-modify-playback-state,user-read-currently-playing";

#[tokio::main]
async fn main() {
    let session_config = SessionConfig {
        user_agent: "just testing".to_string(),
        device_id: "testing device".to_string(),
        proxy: None,
        ap_port: None,
    };

    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: {} USERNAME PASSWORD CLIENT_ID", args[0]);
    }
    let username = args[1].to_owned();
    let password = args[2].to_owned();
    let client_id = &args[3];

    println!("Connecting..");
    let credentials = Credentials::with_password(username, password);
    let session = Session::connect(session_config, credentials, None).await.unwrap();

    println!(
        "Token: {:#?}",
        keymaster::get_token(&session, &client_id, SCOPES)
            .await
            .unwrap()
    );
}
