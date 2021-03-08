#![allow(clippy::unused_io_amount)]
#![deny(missing_docs)]

//! Implements decoding audio with the ogg-vorbis protocol, via either the 
//! [lewton](https://docs.rs/crate/lewton), [vorbis](https://docs.rs/crate/vorbis) or
//! [tremor](https://docs.rs/crate/librespot-tremor) crates. Note that these are mutually exclusive
//! features, and attempting to enable multiple of them will result in a compile error.

#[macro_use]
extern crate log;

mod decrypt;
mod fetch;

use cfg_if::cfg_if;

#[cfg(any(
    all(feature = "with-lewton", feature = "with-tremor"),
    all(feature = "with-vorbis", feature = "with-tremor"),
    all(feature = "with-lewton", feature = "with-vorbis")
))]
compile_error!("Cannot use two decoders at the same time.");

cfg_if! {
    if #[cfg(feature = "with-lewton")] {
        mod lewton_decoder;
        pub use lewton_decoder::{VorbisDecoder, VorbisError};
    } else if #[cfg(any(feature = "with-tremor", feature = "with-vorbis"))] {
        mod libvorbis_decoder;
        pub use crate::libvorbis_decoder::{VorbisDecoder, VorbisError};
    } else {
        compile_error!("Must choose a vorbis decoder.");
    }
}

mod passthrough_decoder;
pub use passthrough_decoder::{PassthroughDecoder, PassthroughError};

mod range_set;

pub use decrypt::AudioDecrypt;
pub use fetch::{AudioFile, StreamLoaderController};
pub use fetch::{
    READ_AHEAD_BEFORE_PLAYBACK_ROUNDTRIPS, READ_AHEAD_BEFORE_PLAYBACK_SECONDS,
    READ_AHEAD_DURING_PLAYBACK_ROUNDTRIPS, READ_AHEAD_DURING_PLAYBACK_SECONDS,
};
use std::fmt;

/// A single packet of audio data.
pub enum AudioPacket {
    /// A list of 16 bit sample data.
    Samples(Vec<i16>),
    /// Ogg encoded sound data.
    OggData(Vec<u8>),
}

impl AudioPacket {
    /// Unwraps the `AudioPacket as `Samples`.
    ///
    /// ### Panics
    /// Panics if the `AudioPacket` is of the variant `OggData`.
    pub fn samples(&self) -> &[i16] {
        match self {
            AudioPacket::Samples(s) => s,
            AudioPacket::OggData(_) => panic!("can't return OggData on samples"),
        }
    }

    /// Unwraps the `AudioPacket as `OggData`.
    ///
    /// ### Panics
    /// Panics if the `AudioPacket` is of the variant `Samples`.
    pub fn oggdata(&self) -> &[u8] {
        match self {
            AudioPacket::Samples(_) => panic!("can't return samples on OggData"),
            AudioPacket::OggData(d) => d,
        }
    }

    /// Returns true if the `AudioPacket` is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            AudioPacket::Samples(s) => s.is_empty(),
            AudioPacket::OggData(d) => d.is_empty(),
        }
    }
}

/// A type representing the ways audio encoding/decoding can fail/
#[derive(Debug)]
pub enum AudioError {
    /// Used when an error occured during the passthough.
    PassthroughError(PassthroughError),
    /// Used when the ogg vorbis decoding failed.
    VorbisError(VorbisError),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AudioError::PassthroughError(err) => write!(f, "PassthroughError({})", err),
            AudioError::VorbisError(err) => write!(f, "VorbisError({})", err),
        }
    }
}

impl From<VorbisError> for AudioError {
    fn from(err: VorbisError) -> AudioError {
        AudioError::VorbisError(err)
    }
}

impl From<PassthroughError> for AudioError {
    fn from(err: PassthroughError) -> AudioError {
        AudioError::PassthroughError(err)
    }
}

/// Represents any type that can be used to decode audio files.
pub trait AudioDecoder {
    /// A `io::Seek` like interface that uses milliseconds instead of bytes as a offset.
    fn seek(&mut self, ms: i64) -> Result<(), AudioError>;

    /// Returns the next packet, if there is one.
    fn next_packet(&mut self) -> Result<Option<AudioPacket>, AudioError>;
}
