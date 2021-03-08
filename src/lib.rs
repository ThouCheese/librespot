#![crate_name = "librespot"]

pub mod audio {
    //! Utilities related to decoding audio packets.

    pub use librespot_audio::*;
}
pub mod connect {
    pub use librespot_connect::*;
}
pub mod core {
    pub use librespot_core::*;
}
pub mod metadata {
    pub use librespot_metadata::*;
}
pub mod playback {
    pub use librespot_playback::*;
}
pub mod protocol {
    pub use librespot_protocol::*;
}
