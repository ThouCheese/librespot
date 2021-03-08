use std::{cmp::max, io::{self, Read, Seek, SeekFrom}, time::Instant};

use futures_util::TryFutureExt;
use librespot_core::{channel::ChannelError, session::Session, spotify_id::FileId};
use tokio::sync::oneshot;

use super::AudioFileStreaming;
use super::{
    INITIAL_DOWNLOAD_SIZE,
    READ_AHEAD_DURING_PLAYBACK_SECONDS,
    INITIAL_PING_TIME_ESTIMATE_SECONDS,
    READ_AHEAD_DURING_PLAYBACK_ROUNDTRIPS,
};

/// Represents a playable audio file, either on disk or incoming over the network.
pub enum AudioFile {
    /// An audio file on disk.
    Cached(std::fs::File),
    /// A audio file that is streamed to this computer.
    Streaming(AudioFileStreaming),
}

impl AudioFile {
    /// todo
    pub async fn open(
        session: &Session,
        file_id: FileId,
        bytes_per_second: usize,
        play_from_beginning: bool,
    ) -> Result<AudioFile, ChannelError> {
        if let Some(file) = session.cache().and_then(|cache| cache.file(file_id)) {
            debug!("File {} already in cache", file_id);
            return Ok(AudioFile::Cached(file));
        }

        debug!("Downloading file {}", file_id);

        let (complete_tx, complete_rx) = oneshot::channel();
        let mut initial_data_length = if play_from_beginning {
            INITIAL_DOWNLOAD_SIZE
                + max(
                    (READ_AHEAD_DURING_PLAYBACK_SECONDS * bytes_per_second as f64) as usize,
                    (INITIAL_PING_TIME_ESTIMATE_SECONDS
                        * READ_AHEAD_DURING_PLAYBACK_ROUNDTRIPS
                        * bytes_per_second as f64) as usize,
                )
        } else {
            INITIAL_DOWNLOAD_SIZE
        };
        if initial_data_length % 4 != 0 {
            initial_data_length += 4 - (initial_data_length % 4);
        }
        let (headers, data) = super::request_range(session, file_id, 0, initial_data_length).split();

        let streaming = AudioFileStreaming::open(
            session.clone(),
            data,
            initial_data_length,
            Instant::now(),
            headers,
            file_id,
            complete_tx,
            bytes_per_second,
        );

        let session_ = session.clone();
        session.spawn(complete_rx.map_ok(move |mut file| {
            if let Some(cache) = session_.cache() {
                debug!("File {} complete, saving to cache", file_id);
                cache.save_file(file_id, &mut file);
            } else {
                debug!("File {} complete", file_id);
            }
        }));

        Ok(AudioFile::Streaming(streaming.await?))
    }

    /// Get a `StreamLoaderController` for this `AudioFile`.
    pub fn get_stream_loader_controller(&self) -> super::StreamLoaderController {
        match self {
            AudioFile::Streaming(ref stream) => super::StreamLoaderController {
                channel_tx: Some(stream.stream_loader_command_tx.clone()),
                stream_shared: Some(stream.shared.clone()),
                file_size: stream.shared.file_size,
            },
            AudioFile::Cached(ref file) => super::StreamLoaderController {
                channel_tx: None,
                stream_shared: None,
                file_size: file.metadata().unwrap().len() as usize,
            },
        }
    }

    /// Returns `true` is this file is cached, or `false` otherwise.
    pub fn is_cached(&self) -> bool {
        matches!(self, AudioFile::Cached( .. ))
    }
}

impl Read for AudioFile {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        match *self {
            AudioFile::Cached(ref mut file) => file.read(output),
            AudioFile::Streaming(ref mut file) => file.read(output),
        }
    }
}

impl Seek for AudioFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match *self {
            AudioFile::Cached(ref mut file) => file.seek(pos),
            AudioFile::Streaming(ref mut file) => file.seek(pos),
        }
    }
}
