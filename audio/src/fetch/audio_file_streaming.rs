// use std::{sync::{self, atomic, mpsc}, time::Instant};
// use std::fs;


// use futures_util::{TryStreamExt, future};
// use librespot_core::{channel::{ChannelData, ChannelError, ChannelHeaders}, session::Session, spotify_id::FileId};
// use tempfile::NamedTempFile;
// use tokio::sync::{mpsc::UnboundedSender, oneshot};
use std::{cmp::{max, min}, fs, io::{self, Read}, sync::{self, atomic}, time::Duration};
use std::io::{Seek, SeekFrom};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use byteorder::{BigEndian, ByteOrder};
use futures_util::{future, StreamExt, TryStreamExt};
use librespot_core::channel::{ChannelData, ChannelError, ChannelHeaders};
use librespot_core::session::Session;
use librespot_core::spotify_id::FileId;
use tempfile::NamedTempFile;
use tokio::sync::{mpsc::{self, UnboundedSender}, oneshot};

use crate::range_set::RangeSet;

use super::{
    AudioFileDownloadStatus, AudioFileFetch, AudioFileShared, DownloadStrategy, StreamLoaderCommand,
    Range,
};
use super::{
    READ_AHEAD_DURING_PLAYBACK_SECONDS,
    READ_AHEAD_DURING_PLAYBACK_ROUNDTRIPS,

};

pub struct AudioFileStreaming {
    pub(crate) read_file: fs::File,
    pub(crate) position: u64,
    pub(crate) stream_loader_command_tx: UnboundedSender<StreamLoaderCommand>,
    pub(crate) shared: sync::Arc<AudioFileShared>,
}

impl AudioFileStreaming {
    pub async fn open(
        session: Session,
        initial_data_rx: ChannelData,
        initial_data_length: usize,
        initial_request_sent_time: Instant,
        headers: ChannelHeaders,
        file_id: FileId,
        complete_tx: oneshot::Sender<NamedTempFile>,
        streaming_data_rate: usize,
    ) -> Result<AudioFileStreaming, ChannelError> {
        let (_, data) = headers
            .try_filter(|(id, _)| future::ready(*id == 0x3))
            .next()
            .await
            .unwrap()?;

        let size = BigEndian::read_u32(&data) as usize * 4;

        let shared = Arc::new(AudioFileShared {
            file_id,
            file_size: size,
            stream_data_rate: streaming_data_rate,
            cond: Condvar::new(),
            download_status: Mutex::new(AudioFileDownloadStatus {
                requested: RangeSet::new(),
                downloaded: RangeSet::new(),
            }),
            download_strategy: Mutex::new(DownloadStrategy::RandomAccess), // start with random access mode until someone tells us otherwise
            number_of_open_requests: AtomicUsize::new(0),
            ping_time_ms: AtomicUsize::new(0),
            read_position: AtomicUsize::new(0),
        });

        let mut write_file = NamedTempFile::new().unwrap();
        write_file.as_file().set_len(size as u64).unwrap();
        write_file.seek(SeekFrom::Start(0)).unwrap();

        let read_file = write_file.reopen().unwrap();

        //let (seek_tx, seek_rx) = mpsc::unbounded();
        let (stream_loader_command_tx, stream_loader_command_rx) =
            mpsc::unbounded_channel::<StreamLoaderCommand>();

        let fetcher = AudioFileFetch::new(
            session.clone(),
            shared.clone(),
            initial_data_rx,
            initial_request_sent_time,
            initial_data_length,
            write_file,
            stream_loader_command_rx,
            complete_tx,
        );

        session.spawn(fetcher);
        Ok(AudioFileStreaming {
            read_file,
            position: 0,
            stream_loader_command_tx,
            shared,
        })
    }
}

impl Read for AudioFileStreaming {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let offset = self.position as usize;

        if offset >= self.shared.file_size {
            return Ok(0);
        }

        let length = min(output.len(), self.shared.file_size - offset);

        let length_to_request = match *(self.shared.download_strategy.lock().unwrap()) {
            DownloadStrategy::RandomAccess => length,
            DownloadStrategy::Streaming => {
                // Due to the read-ahead stuff, we potentially request more than the actual reqeust demanded.
                let ping_time_seconds =
                    0.0001 * self.shared.ping_time_ms.load(atomic::Ordering::Relaxed) as f64;

                let length_to_request = length
                    + max(
                        (READ_AHEAD_DURING_PLAYBACK_SECONDS * self.shared.stream_data_rate as f64)
                            as usize,
                        (READ_AHEAD_DURING_PLAYBACK_ROUNDTRIPS
                            * ping_time_seconds
                            * self.shared.stream_data_rate as f64) as usize,
                    );
                min(length_to_request, self.shared.file_size - offset)
            }
        };

        let mut ranges_to_request = RangeSet::new();
        ranges_to_request += Range::new(offset, length_to_request);

        let mut download_status = self.shared.download_status.lock().unwrap();
        ranges_to_request -= &download_status.downloaded;
        ranges_to_request -= &download_status.requested;

        for range in ranges_to_request {
            self.stream_loader_command_tx
                .send(StreamLoaderCommand::Fetch(range))
                .unwrap();
        }

        if length == 0 {
            return Ok(0);
        }

        let mut download_message_printed = false;
        while !download_status.downloaded.contains(offset) {
            if let DownloadStrategy::Streaming = *self.shared.download_strategy.lock().unwrap() {
                if !download_message_printed {
                    debug!("Stream waiting for download of file position {}. Downloaded ranges: {}. Pending ranges: {}", offset, download_status.downloaded, &download_status.requested - &download_status.downloaded);
                    download_message_printed = true;
                }
            }
            download_status = self
                .shared
                .cond
                .wait_timeout(download_status, Duration::from_millis(1000))
                .unwrap()
                .0;
        }
        let available_length = download_status
            .downloaded
            .contained_length_from_value(offset);
        assert!(available_length > 0);
        drop(download_status);

        self.position = self.read_file.seek(SeekFrom::Start(offset as u64)).unwrap();
        let read_len = min(length, available_length);
        let read_len = self.read_file.read(&mut output[..read_len])?;

        if download_message_printed {
            debug!(
                "Read at postion {} completed. {} bytes returned, {} bytes were requested.",
                offset,
                read_len,
                output.len()
            );
        }

        self.position += read_len as u64;
        self.shared
            .read_position
            .store(self.position as usize, atomic::Ordering::Relaxed);

        Ok(read_len)
    }
}

impl Seek for AudioFileStreaming {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.position = self.read_file.seek(pos)?;
        // Do not seek past EOF
        self.shared
            .read_position
            .store(self.position as usize, atomic::Ordering::Relaxed);
        Ok(self.position)
    }
}
