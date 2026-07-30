use std::fmt;

use aes::{
    Aes128, Aes192, Aes256,
    cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use thiserror::Error;

const AES_BLOCK_SIZE: usize = 16;

/// AES-CBC 加密或解密失败。
#[derive(Error, Debug)]
pub enum AesError {
    #[error("Invalid AES key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("Invalid AES IV length: expected {expected} bytes, got {actual}")]
    InvalidIvLength { expected: usize, actual: usize },
    #[error("AES encryption error: {0}")]
    EncryptError(String),
    #[error("AES decryption error: {0}")]
    DecryptError(String),
    #[error("Base64 decode error: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("Invalid UTF-8 data: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

/// 使用 PKCS#7 填充和 Base64 编码的 AES-CBC 加解密器。
pub struct AesCBCCrypto {
    key: Vec<u8>,
    iv: Vec<u8>,
    key_size: AesKeySize,
}

/// 支持的 AES 密钥长度。
#[derive(Debug, Clone, Copy)]
pub enum AesKeySize {
    Size128,
    Size192,
    Size256,
}

impl fmt::Debug for AesCBCCrypto {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AesCBCCrypto")
            .field("key", &"[REDACTED]")
            .field("iv", &"[REDACTED]")
            .field("key_size", &self.key_size)
            .finish()
    }
}

impl AesCBCCrypto {
    /// 创建一个与指定 AES 密钥长度匹配的 CBC 加解密器。
    ///
    /// # Errors
    ///
    /// 当密钥长度与 `key_size` 不匹配或 IV 不是 16 字节时返回错误。
    pub fn new(key: &str, iv: &str, key_size: AesKeySize) -> Result<Self, AesError> {
        let expected_key_length = match key_size {
            AesKeySize::Size128 => 16,
            AesKeySize::Size192 => 24,
            AesKeySize::Size256 => 32,
        };
        if key.len() != expected_key_length {
            return Err(AesError::InvalidKeyLength {
                expected: expected_key_length,
                actual: key.len(),
            });
        }

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

    /// 加密 UTF-8 字符串并返回 Base64 编码的密文。
    ///
    /// # Errors
    ///
    /// 当内部密码器初始化失败时返回错误。
    pub fn encrypt(&self, data: &str) -> Result<String, AesError> {
        let encrypted = match self.key_size {
            AesKeySize::Size128 => cbc::Encryptor::<Aes128>::new_from_slices(&self.key, &self.iv)
                .map_err(|error| AesError::EncryptError(error.to_string()))?
                .encrypt_padded_vec::<Pkcs7>(data.as_bytes()),
            AesKeySize::Size192 => cbc::Encryptor::<Aes192>::new_from_slices(&self.key, &self.iv)
                .map_err(|error| AesError::EncryptError(error.to_string()))?
                .encrypt_padded_vec::<Pkcs7>(data.as_bytes()),
            AesKeySize::Size256 => cbc::Encryptor::<Aes256>::new_from_slices(&self.key, &self.iv)
                .map_err(|error| AesError::EncryptError(error.to_string()))?
                .encrypt_padded_vec::<Pkcs7>(data.as_bytes()),
        };

        Ok(STANDARD.encode(encrypted))
    }

    /// 解密 Base64 编码的 AES-CBC 密文。
    ///
    /// # Errors
    ///
    /// 当输入不是有效 Base64、密文填充无效、密码器初始化失败或明文不是 UTF-8 时返回错误。
    pub fn decrypt(&self, encrypted_base64: &str) -> Result<String, AesError> {
        let encrypted_data = STANDARD
            .decode(encrypted_base64)
            .map_err(AesError::Base64DecodeError)?;
        let decrypted = match self.key_size {
            AesKeySize::Size128 => cbc::Decryptor::<Aes128>::new_from_slices(&self.key, &self.iv)
                .map_err(|error| AesError::DecryptError(error.to_string()))?
                .decrypt_padded_vec::<Pkcs7>(&encrypted_data)
                .map_err(|error| AesError::DecryptError(error.to_string()))?,
            AesKeySize::Size192 => cbc::Decryptor::<Aes192>::new_from_slices(&self.key, &self.iv)
                .map_err(|error| AesError::DecryptError(error.to_string()))?
                .decrypt_padded_vec::<Pkcs7>(&encrypted_data)
                .map_err(|error| AesError::DecryptError(error.to_string()))?,
            AesKeySize::Size256 => cbc::Decryptor::<Aes256>::new_from_slices(&self.key, &self.iv)
                .map_err(|error| AesError::DecryptError(error.to_string()))?
                .decrypt_padded_vec::<Pkcs7>(&encrypted_data)
                .map_err(|error| AesError::DecryptError(error.to_string()))?,
        };

        String::from_utf8(decrypted).map_err(AesError::Utf8Error)
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
    fn test_should_preserve_legacy_aes_cbc_ciphertext_format() -> Result<()> {
        let crypto =
            AesCBCCrypto::new("1234567890123456", "1234567890123456", AesKeySize::Size128)?;
        let legacy_ciphertext = "N7xB4AmTENuP6jN7K71JZkxfVjy9V7d9BXwd1ptfHPg=";

        assert_eq!(crypto.encrypt("legacy-token-fixture")?, legacy_ciphertext);
        assert_eq!(crypto.decrypt(legacy_ciphertext)?, "legacy-token-fixture",);
        Ok(())
    }

    #[test]
    fn test_should_redact_key_and_iv_from_debug_output() -> Result<()> {
        let key = "1234567890123456";
        let iv = "abcdefghijklmnop";
        let crypto = AesCBCCrypto::new(key, iv, AesKeySize::Size128)?;

        let debug_output = format!("{crypto:?}");

        assert!(!debug_output.contains(key));
        assert!(!debug_output.contains(iv));
        assert!(debug_output.contains("[REDACTED]"));
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
