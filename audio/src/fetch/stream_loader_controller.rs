use std::sync::{Arc, atomic};
use std::time::Duration;

use tokio::sync::mpsc;

use crate::range_set::Range;

use super::audio_file_shared::AudioFileShared;

#[derive(Debug)]
pub(crate) enum StreamLoaderCommand {
    Fetch(Range),     // signal the stream loader to fetch a range of the file
    RandomAccessMode, // optimise download strategy for random access
    StreamMode,       // optimise download strategy for streaming
    Close,            // terminate and don't load any more data
}

/// A 
#[derive(Clone)]
pub struct StreamLoaderController {
    pub(crate) channel_tx: Option<mpsc::UnboundedSender<StreamLoaderCommand>>,
    pub(crate) stream_shared: Option<Arc<AudioFileShared>>,
    pub(crate) file_size: usize,
}

impl StreamLoaderController {
    /// Returns the length of the stream.
    pub fn len(&self) -> usize {
        self.file_size
    }

    /// Returns `true` if the stream is empty, or `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.file_size == 0
    }

    /// Returns `true` if the current `StreamLoaderController` is still able to produce the bytes
    /// contained in `range`.
    pub fn range_available(&self, range: Range) -> bool {
        if let Some(ref shared) = self.stream_shared {
            let download_status = shared.download_status.lock().unwrap();
            range.len()
                <= download_status
                    .downloaded
                    .contained_length_from_value(range.start())
        } else {
            range.len() <= self.len() - range.start()
        }
    }

    /// Returns true is it is possible to read the full stream right now. This always returns true
    /// for non-shared streams.
    pub fn range_to_end_available(&self) -> bool {
        self.stream_shared.as_ref().map_or(true, |shared| {
            let read_position = shared.read_position.load(atomic::Ordering::Relaxed);
            self.range_available(Range::new(read_position, self.len() - read_position))
        })
    }

    /// Returns the ping in miliseconds 
    pub fn ping_time_ms(&self) -> usize {
        self.stream_shared.as_ref().map_or(0, |shared| {
            shared.ping_time_ms.load(atomic::Ordering::Relaxed)
        })
    }

    fn send_stream_loader_command(&mut self, command: StreamLoaderCommand) {
        if let Some(ref mut channel) = self.channel_tx {
            // ignore the error in case the channel has been closed already.
            let _ = channel.send(command);
        }
    }

    /// Retrieve the data between the bytes specified by `range` from the stream in a non-blocking
    /// way.
    pub fn fetch(&mut self, range: Range) {
        // signal the stream loader to fetch a range of the file
        self.send_stream_loader_command(StreamLoaderCommand::Fetch(range));
    }

    /// Retrieve the data between the bytes specified by `range` from the stream in a blocking way.
    pub fn fetch_blocking(&mut self, mut range: Range) {
        // signal the stream loader to tech a range of the file and block until it is loaded.

        // ensure the range is within the file's bounds.
        if range.start() >= self.len() {
            range.clear();
        } else if range.end() > self.len() {
            range.set_len(self.len() - range.start());
        }

        self.fetch(range);

        if let Some(ref shared) = self.stream_shared {
            let mut download_status = shared.download_status.lock().unwrap();
            while range.len()
                > download_status
                    .downloaded
                    .contained_length_from_value(range.start())
            {
                download_status = shared
                    .cond
                    .wait_timeout(download_status, Duration::from_millis(1000))
                    .unwrap()
                    .0;
                if range.len()
                    > (&download_status.downloaded + &download_status.requested)
                        .contained_length_from_value(range.start())
                {
                    // For some reason, the requested range is neither downloaded nor requested.
                    // This could be due to a network error. Request it again.
                    // We can't use self.fetch here because self can't be borrowed mutably, so we access the channel directly.
                    if let Some(ref mut channel) = self.channel_tx {
                        // ignore the error in case the channel has been closed already.
                        let _ = channel.send(StreamLoaderCommand::Fetch(range));
                    }
                }
            }
        }
    }

    /// Retrieve the data starting at the current position until `length` from the stream in a
    /// non-blocking way.
    pub fn fetch_next(&mut self, length: usize) {
        if let Some(ref shared) = self.stream_shared {
            let start = shared.read_position.load(atomic::Ordering::Relaxed);
            let range = Range::new(start, length);
            self.fetch(range)
        }
    }

    /// Retrieve the data starting at the current position until `length` from the stream in a
    /// blocking way.
    pub fn fetch_next_blocking(&mut self, length: usize) {
        if let Some(ref shared) = self.stream_shared {
            let start = shared.read_position.load(atomic::Ordering::Relaxed);
            let range = Range::new(start, length);
            self.fetch_blocking(range);
        }
    }

    /// Overwrite the access mode to be random access.
    pub fn set_random_access_mode(&mut self) {
        // optimise download strategy for random access
        self.send_stream_loader_command(StreamLoaderCommand::RandomAccessMode);
    }

    /// Overwrite the access mode to be streaming.
    pub fn set_stream_mode(&mut self) {
        // optimise download strategy for streaming
        self.send_stream_loader_command(StreamLoaderCommand::StreamMode);
    }

    /// Close the underlying stream.
    pub fn close(&mut self) {
        // terminate stream loading and don't load any more data for this file.
        self.send_stream_loader_command(StreamLoaderCommand::Close);
    }
}