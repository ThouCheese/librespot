use lewton::inside_ogg::OggStreamReader;

use super::{AudioDecoder, AudioError, AudioPacket};
use std::error;
use std::fmt;
use std::io::{Read, Seek};

/// A wrapper struct around a stream that yields decoded data rather than raw data.
pub struct VorbisDecoder<R: Read + Seek>(OggStreamReader<R>);

/// The error returned when decoding fails.
pub struct VorbisError(lewton::VorbisError);

impl<R> VorbisDecoder<R>
where
    R: Read + Seek,
{
    /// Construct a new `VorbisDecoder` from an object that implements `Read` and `Seek`.
    ///
    /// ### Example
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use librespot_audio::VorbisDecoder;
    /// use std::fs::File;
    ///
    /// let f = File::open("/some/file.txt")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(input: R) -> Result<VorbisDecoder<R>, VorbisError> {
        Ok(VorbisDecoder(OggStreamReader::new(input)?))
    }
}

impl<R> AudioDecoder for VorbisDecoder<R>
where
    R: Read + Seek,
{
    fn seek(&mut self, ms: i64) -> Result<(), AudioError> {
        let absgp = ms * 44100 / 1000;
        match self.0.seek_absgp_pg(absgp as u64) {
            Ok(_) => Ok(()),
            Err(err) => Err(AudioError::VorbisError(err.into())),
        }
    }

    fn next_packet(&mut self) -> Result<Option<AudioPacket>, AudioError> {
        use lewton::audio::AudioReadError::AudioIsHeader;
        use lewton::OggReadError::NoCapturePatternFound;
        use lewton::VorbisError::BadAudio;
        use lewton::VorbisError::OggError;
        loop {
            match self.0.read_dec_packet_itl() {
                Ok(Some(packet)) => return Ok(Some(AudioPacket::Samples(packet))),
                Ok(None) => return Ok(None),

                Err(BadAudio(AudioIsHeader)) => (),
                Err(OggError(NoCapturePatternFound)) => (),
                Err(err) => return Err(AudioError::VorbisError(err.into())),
            }
        }
    }
}

impl From<lewton::VorbisError> for VorbisError {
    fn from(err: lewton::VorbisError) -> VorbisError {
        VorbisError(err)
    }
}

impl fmt::Debug for VorbisError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for VorbisError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl error::Error for VorbisError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        error::Error::source(&self.0)
    }
}
