use std::sync::{self, atomic};

use librespot_core::spotify_id::FileId;

use super::{AudioFileDownloadStatus, DownloadStrategy};

pub(crate) struct AudioFileShared {
    pub(crate) file_id: FileId,
    pub(crate) file_size: usize,
    pub(crate) stream_data_rate: usize,
    pub(crate) cond: sync::Condvar,
    pub(crate) download_status: sync::Mutex<AudioFileDownloadStatus>,
    pub(crate) download_strategy: sync::Mutex<DownloadStrategy>,
    pub(crate) number_of_open_requests: atomic::AtomicUsize,
    pub(crate) ping_time_ms: atomic::AtomicUsize,
    pub(crate) read_position: atomic::AtomicUsize,
}

