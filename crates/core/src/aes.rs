use base64::{Engine, engine::general_purpose::STANDARD};
use crypto::{
    aes, blockmodes,
    buffer::{self, BufferResult, ReadBuffer, WriteBuffer},
    symmetriccipher::SymmetricCipherError,
};
use thiserror::Error;

/// 自定义 AES 加密解密过程中可能发生的错误类型。
///
/// 由于 `rust-crypto` 库中的 `SymmetricCipherError` 仅实现了 `Debug` 而未实现 `std::fmt::Display`，
/// 在 `thiserror` 的 `#[error(...)]` 属性中需要显式指定使用 `{:?}` 调用 `Debug` 格式化，
/// 以解决 `SymmetricCipherError: std::fmt::Display` 未满足的编译错误。
#[derive(Error, Debug)]
pub enum AesError {
    #[error("Invalid AES key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("Invalid AES IV length: expected {expected} bytes, got {actual}")]
    InvalidIvLength { expected: usize, actual: usize },
    #[error("AES encryption error: {0:?}")]
    EncryptError(SymmetricCipherError),
    #[error("AES decryption error: {0:?}")]
    DecryptError(SymmetricCipherError),
    #[error("Base64 decode error: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("Invalid UTF-8 data: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

pub struct AesCBCCrypto {
    key: Vec<u8>,
    iv: Vec<u8>,
    key_size: aes::KeySize,
}

pub enum AesKeySize {
    Size128,
    Size192,
    Size256,
}

impl AesCBCCrypto {
    pub fn new(key: &str, iv: &str, key_size: AesKeySize) -> Result<Self, AesError> {
        let (key_size, expected_key_length) = match key_size {
            AesKeySize::Size128 => (aes::KeySize::KeySize128, 16),
            AesKeySize::Size192 => (aes::KeySize::KeySize192, 24),
            AesKeySize::Size256 => (aes::KeySize::KeySize256, 32),
        };
        if key.len() != expected_key_length {
            return Err(AesError::InvalidKeyLength {
                expected: expected_key_length,
                actual: key.len(),
            });
        }

        const AES_BLOCK_SIZE: usize = 16;
        if iv.len() != AES_BLOCK_SIZE {
            return Err(AesError::InvalidIvLength {
                expected: AES_BLOCK_SIZE,
                actual: iv.len(),
            });
        }

        Ok(Self {
            key: key.as_bytes().into(),
            iv: iv.as_bytes().into(),
            key_size,
        })
    }

    pub fn encrypt(&self, data: &str) -> Result<String, AesError> {
        let mut encryptor =
            aes::cbc_encryptor(self.key_size, &self.key, &self.iv, blockmodes::PkcsPadding);
        let mut final_result = Vec::<u8>::new();
        let mut read_buffer = buffer::RefReadBuffer::new(data.as_bytes());
        let mut buffer = [0; 4096];
        let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

        loop {
            // 将 SymmetricCipherError 显式映射为 AesError，从而能够透明传播给 anyhow::Result
            let result = encryptor
                .encrypt(&mut read_buffer, &mut write_buffer, true)
                .map_err(AesError::EncryptError)?;

            // "write_buffer.take_read_buffer().take_remaining()"
            // 表示从可写缓冲区提取已写入的数据切片并追加到最终结果中
            final_result.extend(
                write_buffer
                    .take_read_buffer()
                    .take_remaining()
                    .iter()
                    .copied(),
            );

            match result {
                BufferResult::BufferUnderflow => break,
                BufferResult::BufferOverflow => {}
            }
        }

        Ok(STANDARD.encode(final_result))
    }

    pub fn decrypt(&self, encrypted_base64: &str) -> Result<String, AesError> {
        let encrypted_data = STANDARD
            .decode(encrypted_base64)
            .map_err(AesError::Base64DecodeError)?;
        let mut decryptor =
            aes::cbc_decryptor(self.key_size, &self.key, &self.iv, blockmodes::PkcsPadding);
        let mut final_result = Vec::<u8>::new();
        let mut read_buffer = buffer::RefReadBuffer::new(&encrypted_data);
        let mut buffer = [0; 4096];
        let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

        loop {
            let result = decryptor
                .decrypt(&mut read_buffer, &mut write_buffer, true)
                .map_err(AesError::DecryptError)?;

            final_result.extend(
                write_buffer
                    .take_read_buffer()
                    .take_remaining()
                    .iter()
                    .copied(),
            );

            match result {
                BufferResult::BufferUnderflow => break,
                BufferResult::BufferOverflow => {}
            }
        }

        String::from_utf8(final_result).map_err(AesError::Utf8Error)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use base64::{Engine, engine::general_purpose::STANDARD};

    use super::*;

    #[test]
    fn test_should_encrypt_and_decrypt_successfully() -> Result<()> {
        let key = "1234567890123456";
        let iv = "1234567890123456";
        let crypto = AesCBCCrypto::new(key, iv, AesKeySize::Size128)?;

        let plaintext = "Hello Rust AES Encryption!";
        let ciphertext = crypto.encrypt(plaintext)?;
        let decrypted = crypto.decrypt(&ciphertext)?;

        assert_eq!(plaintext, decrypted);
        Ok(())
    }

    #[test]
    fn test_should_support_all_aes_key_sizes() -> Result<()> {
        for (key, key_size) in [
            ("1234567890123456", AesKeySize::Size128),
            ("123456789012345678901234", AesKeySize::Size192),
            ("12345678901234567890123456789012", AesKeySize::Size256),
        ] {
            let crypto = AesCBCCrypto::new(key, "1234567890123456", key_size)?;
            let ciphertext = crypto.encrypt("key-size-round-trip")?;
            assert_eq!(crypto.decrypt(&ciphertext)?, "key-size-round-trip");
        }

        Ok(())
    }

    #[test]
    fn test_should_round_trip_empty_unicode_and_large_plaintext() -> Result<()> {
        let crypto = AesCBCCrypto::new(
            "12345678901234567890123456789012",
            "1234567890123456",
            AesKeySize::Size256,
        )?;

        for plaintext in [
            String::new(),
            "你好，Rust 🦀".to_string(),
            "a".repeat(8_193),
        ] {
            let ciphertext = crypto.encrypt(&plaintext)?;
            assert_eq!(crypto.decrypt(&ciphertext)?, plaintext);
        }

        Ok(())
    }

    #[test]
    fn test_should_reject_invalid_base64_ciphertext() -> Result<()> {
        let crypto = AesCBCCrypto::new(
            "12345678901234567890123456789012",
            "1234567890123456",
            AesKeySize::Size256,
        )?;

        assert!(matches!(
            crypto.decrypt("not valid base64!"),
            Err(AesError::Base64DecodeError(_))
        ));
        Ok(())
    }

    #[test]
    fn test_should_reject_corrupted_ciphertext() -> Result<()> {
        let crypto = AesCBCCrypto::new(
            "12345678901234567890123456789012",
            "1234567890123456",
            AesKeySize::Size256,
        )?;
        let ciphertext = crypto.encrypt("integrity-check")?;
        let mut encrypted = STANDARD.decode(ciphertext)?;
        let Some(last_byte) = encrypted.last_mut() else {
            return Err(anyhow::anyhow!("ciphertext must not be empty"));
        };
        *last_byte ^= 0xff;
        let corrupted = STANDARD.encode(encrypted);

        assert!(matches!(
            crypto.decrypt(&corrupted),
            Err(AesError::DecryptError(_))
        ));
        Ok(())
    }

    #[test]
    fn test_should_reject_invalid_key_or_iv_length() {
        assert!(AesCBCCrypto::new("short", "1234567890123456", AesKeySize::Size128,).is_err());
        assert!(AesCBCCrypto::new("1234567890123456", "short", AesKeySize::Size128,).is_err());
    }
}
