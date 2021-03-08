use std::io::{self, Read};

use aes::Aes192;
use byteorder::{BigEndian, ByteOrder};
use hmac::Hmac;
use pbkdf2::pbkdf2;
use serde::{Deserialize, Serialize};
use protobuf::ProtobufEnum;
use sha1::{Digest, Sha1};

use crate::protocol::authentication::AuthenticationType;

/// The credentials are used to log into the Spotify API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    #[serde(with = "protobuf_enum")]
    pub auth_type: AuthenticationType,
    #[serde(alias = "encoded_auth_blob")]
    #[serde(with = "base_64")]
    pub auth_data: Vec<u8>,
}

impl Credentials {
    /// Intialize these credentials from a username and a password.
    ///
    /// ### Example
    /// ```rust
    /// use librespot_core::authentication::Credentials;
    /// 
    /// let creds = Credentials::with_password("my account", "my password");
    /// ```
    pub fn with_password(
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Credentials {
        Self {
            username: username.into(),
            auth_type: AuthenticationType::AUTHENTICATION_USER_PASS,
            auth_data: password.into().into_bytes(),
        }
    }

    /// Retrieve the credentials from an encrypted blob, using `device_id` as the secret key.
    ///
    /// ### Example
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let blob = "some blob";
    /// use librespot_core::authentication::Credentials;
    /// 
    /// Credentials::with_blob("my account", blob, "the device id")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_blob(
        username: impl Into<String>,
        encrypted_blob: &str,
        device_id: &str,
    ) -> io::Result<Credentials> {
        let username = username.into();

        fn read_u8<R: Read>(stream: &mut R) -> io::Result<u8> {
            let mut data = [0u8];
            stream.read_exact(&mut data)?;
            Ok(data[0])
        }

        fn read_int<R: Read>(stream: &mut R) -> io::Result<u32> {
            let lo = read_u8(stream)? as u32;
            if lo & 0x80 == 0 {
                return Ok(lo);
            }

            let hi = read_u8(stream)? as u32;
            Ok(lo & 0x7f | hi << 7)
        }

        fn read_bytes<R: Read>(stream: &mut R) -> io::Result<Vec<u8>> {
            let length = read_int(stream)?;
            let mut data = vec![0u8; length as usize];
            stream.read_exact(&mut data)?;

            Ok(data)
        }

        let secret = Sha1::digest(device_id.as_bytes());

        let key = {
            let mut key = [0u8; 24];
            pbkdf2::<Hmac<Sha1>>(&secret, username.as_bytes(), 0x100, &mut key[0..20]);

            let hash = &Sha1::digest(&key[..20]);
            key[..20].copy_from_slice(hash);
            BigEndian::write_u32(&mut key[20..], 20);
            key
        };

        // decrypt data using ECB mode without padding
        let blob = {
            use aes::cipher::generic_array::typenum::Unsigned;
            use aes::cipher::generic_array::GenericArray;
            use aes::cipher::{BlockCipher, NewBlockCipher};

            let mut data = base64::decode(encrypted_blob)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            let cipher = Aes192::new(GenericArray::from_slice(&key));
            let block_size = <Aes192 as BlockCipher>::BlockSize::to_usize();
            assert_eq!(data.len() % block_size, 0);
            // replace to chunks_exact_mut with MSRV bump to 1.31
            for chunk in data.chunks_mut(block_size) {
                cipher.decrypt_block(GenericArray::from_mut_slice(chunk));
            }

            let l = data.len();
            for i in 0..l - 0x10 {
                data[l - i - 1] ^= data[l - i - 0x11];
            }

            data
        };

        let mut cursor = io::Cursor::new(&blob);
        read_u8(&mut cursor)?;
        read_bytes(&mut cursor)?;
        read_u8(&mut cursor)?;
        let auth_type = read_int(&mut cursor)?;
        let auth_type = AuthenticationType::from_i32(auth_type as i32).unwrap();
        read_u8(&mut cursor)?;
        let auth_data = read_bytes(&mut cursor)?;

        let creds = Self {
            username: username.into(),
            auth_type,
            auth_data,
        };
        Ok(creds)
    }
}

mod protobuf_enum {
    use super::ProtobufEnum;

    pub fn serialize<T, S>(v: &T, ser: S) -> Result<S::Ok, S::Error>
    where
        T: ProtobufEnum,
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&v.value(), ser)
    }

    pub fn deserialize<'de, T, D>(de: D) -> Result<T, D::Error>
    where
        T: ProtobufEnum,
        D: serde::Deserializer<'de>,
    {
        let v: i32 = serde::Deserialize::deserialize(de)?;
        T::from_i32(v).ok_or_else(|| serde::de::Error::custom("Invalid enum value"))
    }
}

mod base_64 {
    pub fn serialize<T, S>(v: &T, ser: S) -> Result<S::Ok, S::Error>
    where
        T: AsRef<[u8]>,
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&base64::encode(v.as_ref()), ser)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v: String = serde::Deserialize::deserialize(de)?;
        base64::decode(&v).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}
