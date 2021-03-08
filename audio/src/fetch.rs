use std::io::Write;

use byteorder::{BigEndian, WriteBytesExt};
use librespot_core::channel::Channel;
use librespot_core::session::Session;
use librespot_core::spotify_id::FileId;

use crate::range_set::{Range, RangeSet};

/// The minimum size of a block that is requested from the Spotify servers in one request.
/// This is the block size that is typically requested while doing a seek() on a file.
/// Note: smaller requests can happen if part of the block is downloaded already.
const MINIMUM_DOWNLOAD_SIZE: usize = 1024 * 16;

/// The amount of data that is requested when initially opening a file.
/// Note: if the file is opened to play from the beginning, the amount of data to
/// read ahead is requested in addition to this amount. If the file is opened to seek to
/// another position, then only this amount is requested on the first request.
const INITIAL_DOWNLOAD_SIZE: usize = 1024 * 16;

/// The pig time that is used for calculations before a ping time was actually measured.
const INITIAL_PING_TIME_ESTIMATE_SECONDS: f64 = 0.5;

/// If the measured ping time to the Spotify server is larger than this value, it is capped
/// to avoid run-away block sizes and pre-fetching.
const MAXIMUM_ASSUMED_PING_TIME_SECONDS: f64 = 1.5;

/// Before playback starts, this many seconds of data must be present.
/// Note: the calculations are done using the nominal bitrate of the file. The actual amount
/// of audio data may be larger or smaller.
pub const READ_AHEAD_BEFORE_PLAYBACK_SECONDS: f64 = 1.0;

/// Same as READ_AHEAD_BEFORE_PLAYBACK_SECONDS, but the time is taken as a factor of the ping
/// time to the Spotify server.
/// Both, READ_AHEAD_BEFORE_PLAYBACK_SECONDS and READ_AHEAD_BEFORE_PLAYBACK_ROUNDTRIPS are
/// obeyed.
/// Note: the calculations are done using the nominal bitrate of the file. The actual amount
/// of audio data may be larger or smaller.
pub const READ_AHEAD_BEFORE_PLAYBACK_ROUNDTRIPS: f64 = 2.0;

/// While playing back, this many seconds of data ahead of the current read position are
/// requested.
/// Note: the calculations are done using the nominal bitrate of the file. The actual amount
/// of audio data may be larger or smaller.
pub const READ_AHEAD_DURING_PLAYBACK_SECONDS: f64 = 5.0;

/// Same as READ_AHEAD_DURING_PLAYBACK_SECONDS, but the time is taken as a factor of the ping
/// time to the Spotify server.
/// Note: the calculations are done using the nominal bitrate of the file. The actual amount
/// of audio data may be larger or smaller.
pub const READ_AHEAD_DURING_PLAYBACK_ROUNDTRIPS: f64 = 10.0;

/// If the amount of data that is pending (requested but not received) is less than a certain
/// amount, data is pre-fetched in addition to the read ahead settings above. The threshold for
/// requesting more data is calculated as
/// <pending bytes> < PREFETCH_THRESHOLD_FACTOR * <ping time> * <nominal data rate>
const PREFETCH_THRESHOLD_FACTOR: f64 = 4.0;

/// Similar to PREFETCH_THRESHOLD_FACTOR, but it also takes the current download rate into account.
/// The formula used is
/// <pending bytes> < FAST_PREFETCH_THRESHOLD_FACTOR * <ping time> * <measured download rate>
/// This mechanism allows for fast downloading of the remainder of the file. The number should be
/// larger than 1 so the download rate ramps up until the bandwidth is saturated. The larger the
/// value, the faster the download rate ramps up. However, this comes at the cost that it might hurt
/// ping-time if a seek is performed while downloading. Values smaller than 1 cause the download
/// rate to collapse and effectively only PREFETCH_THRESHOLD_FACTOR is in effect. Thus, set to zero
/// if bandwidth saturation is not wanted.
const FAST_PREFETCH_THRESHOLD_FACTOR: f64 = 1.5;

/// Limit the number of requests that are pending simultaneously before pre-fetching data. Pending
/// requests share bandwidth. Thus, havint too many requests can lead to the one that is needed next
/// for playback to be delayed leading to a buffer underrun. This limit has the effect that a new
/// pre-fetch request is only sent if less than MAX_PREFETCH_REQUESTS are pending.
const MAX_PREFETCH_REQUESTS: usize = 4;

mod audio_file;
mod audio_file_fetch;
mod audio_file_streaming;
mod audio_file_shared;
mod stream_loader_controller;
use {
    audio_file_fetch::AudioFileFetch,
    audio_file_streaming::AudioFileStreaming,
    audio_file_shared::AudioFileShared,
    stream_loader_controller::StreamLoaderCommand,
};
pub use {
    audio_file::AudioFile,
    stream_loader_controller::{StreamLoaderController},
};

pub(crate) struct AudioFileDownloadStatus {
    requested: RangeSet,
    downloaded: RangeSet,
}

#[derive(Copy, Clone)]
pub(crate) enum DownloadStrategy {
    RandomAccess,
    Streaming,
}

fn request_range(session: &Session, file: FileId, offset: usize, length: usize) -> Channel {
    assert!(
        offset % 4 == 0,
        "Range request start positions must be aligned by 4 bytes."
    );
    assert!(
        length % 4 == 0,
        "Range request range lengths must be aligned by 4 bytes."
    );
    let start = offset / 4;
    let end = (offset + length) / 4;

    let (id, channel) = session.channel().allocate();

    let mut data: Vec<u8> = Vec::new();
    data.write_u16::<BigEndian>(id).unwrap();
    data.write_u8(0).unwrap();
    data.write_u8(1).unwrap();
    data.write_u16::<BigEndian>(0x0000).unwrap();
    data.write_u32::<BigEndian>(0x00000000).unwrap();
    data.write_u32::<BigEndian>(0x00009C40).unwrap();
    data.write_u32::<BigEndian>(0x00020000).unwrap();
    data.write(&file.0).unwrap();
    data.write_u32::<BigEndian>(start as u32).unwrap();
    data.write_u32::<BigEndian>(end as u32).unwrap();

    session.send_packet(0x8, data);

    channel
}


/*
async fn audio_file_fetch(
    session: Session,
    shared: Arc<AudioFileShared>,
    initial_data_rx: ChannelData,
    initial_request_sent_time: Instant,
    initial_data_length: usize,

    output: NamedTempFile,
    stream_loader_command_rx: mpsc::UnboundedReceiver<StreamLoaderCommand>,
    complete_tx: oneshot::Sender<NamedTempFile>,
) {
    let (file_data_tx, file_data_rx) = unbounded::<ReceivedData>();

    let requested_range = Range::new(0, initial_data_length);
    let mut download_status = shared.download_status.lock().unwrap();
    download_status.requested.add_range(&requested_range);

    session.spawn(audio_file_fetch_receive_data(
        shared.clone(),
        file_data_tx.clone(),
        initial_data_rx,
        0,
        initial_data_length,
        initial_request_sent_time,
    ));

    let mut network_response_times_ms: Vec::new();

    let f1 = file_data_rx.map(|x| Ok::<_, ()>(x)).try_for_each(|x| {
        match x {
            ReceivedData::ResponseTimeMs(response_time_ms) => {
                trace!("Ping time estimated as: {} ms.", response_time_ms);

                // record the response time
                network_response_times_ms.push(response_time_ms);

                // prune old response times. Keep at most three.
                while network_response_times_ms.len() > 3 {
                    network_response_times_ms.remove(0);
                }

                // stats::median is experimental. So we calculate the median of up to three ourselves.
                let ping_time_ms: usize = match network_response_times_ms.len() {
                    1 => network_response_times_ms[0] as usize,
                    2 => {
                        ((network_response_times_ms[0] + network_response_times_ms[1]) / 2) as usize
                    }
                    3 => {
                        let mut times = network_response_times_ms.clone();
                        times.sort();
                        times[1]
                    }
                    _ => unreachable!(),
                };

                // store our new estimate for everyone to see
                shared
                    .ping_time_ms
                    .store(ping_time_ms, atomic::Ordering::Relaxed);
            }
            ReceivedData::Data(data) => {
                output
                    .as_mut()
                    .unwrap()
                    .seek(SeekFrom::Start(data.offset as u64))
                    .unwrap();
                output
                    .as_mut()
                    .unwrap()
                    .write_all(data.data.as_ref())
                    .unwrap();

                let mut full = false;

                {
                    let mut download_status = shared.download_status.lock().unwrap();

                    let received_range = Range::new(data.offset, data.data.len());
                    download_status.downloaded.add_range(&received_range);
                    shared.cond.notify_all();

                    if download_status.downloaded.contained_length_from_value(0)
                        >= shared.file_size
                    {
                        full = true;
                    }

                    drop(download_status);
                }

                if full {
                    self.finish();
                    return future::ready(Err(()));
                }
            }
        }
        future::ready(Ok(()))
    });

    let f2 = stream_loader_command_rx.map(Ok::<_, ()>).try_for_each(|x| {
        match cmd {
            StreamLoaderCommand::Fetch(request) => {
                self.download_range(request.start(), request.length);
            }
            StreamLoaderCommand::RandomAccessMode() => {
                *(shared.download_strategy.lock().unwrap()) = DownloadStrategy::RandomAccess;
            }
            StreamLoaderCommand::StreamMode() => {
                *(shared.download_strategy.lock().unwrap()) = DownloadStrategy::Streaming;
            }
            StreamLoaderCommand::Close() => return future::ready(Err(())),
        }
        Ok(())
    });

    let f3 = future::poll_fn(|_| {
        if let DownloadStrategy::Streaming = self.get_download_strategy() {
            let number_of_open_requests = shared
                .number_of_open_requests
                .load(atomic::Ordering::SeqCst);
            let max_requests_to_send =
                MAX_PREFETCH_REQUESTS - min(MAX_PREFETCH_REQUESTS, number_of_open_requests);

            if max_requests_to_send > 0 {
                let bytes_pending: usize = {
                    let download_status = shared.download_status.lock().unwrap();
                    download_status
                        .requested
                        .minus(&download_status.downloaded)
                        .len()
                };

                let ping_time_seconds =
                    0.001 * shared.ping_time_ms.load(atomic::Ordering::Relaxed) as f64;
                let download_rate = session.channel().get_download_rate_estimate();

                let desired_pending_bytes = max(
                    (PREFETCH_THRESHOLD_FACTOR * ping_time_seconds * shared.stream_data_rate as f64)
                        as usize,
                    (FAST_PREFETCH_THRESHOLD_FACTOR * ping_time_seconds * download_rate as f64)
                        as usize,
                );

                if bytes_pending < desired_pending_bytes {
                    self.pre_fetch_more_data(
                        desired_pending_bytes - bytes_pending,
                        max_requests_to_send,
                    );
                }
            }
        }
        Poll::Pending
    });
    future::select_all(vec![f1, f2, f3]).await
}*/
