//! Byte-level crypto helpers behind the `crypto.*` script API.
//!
//! Output formats match the previous WebCrypto implementation: hashes are
//! lowercase hex, AES ciphertexts are `iv || ciphertext(+tag)` encoded as
//! Base64 (or hex for the `*Hex` variants).

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Aes256Gcm, AesGcm, Nonce};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use md5::Md5;
use rand::RngCore;
use sha1::Sha1;
use sha2::{Digest, Sha256};

type Aes192Gcm = AesGcm<aes::Aes192, aes_gcm::aead::consts::U12>;

pub const GCM_IV_LEN: usize = 12;
pub const CBC_IV_LEN: usize = 16;

pub fn base64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Lenient Base64 decoding: standard or URL-safe alphabet, padding optional,
/// whitespace ignored.
pub fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(&cleaned) {
            return Ok(bytes);
        }
    }
    Err(format!("无效的 Base64 字符串: {}", truncate(input)))
}

pub fn hex_encode(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

pub fn hex_decode(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let cleaned = cleaned.strip_prefix("0x").unwrap_or(&cleaned);
    hex::decode(cleaned).map_err(|e| format!("无效的 Hex 字符串 ({e}): {}", truncate(input)))
}

fn truncate(s: &str) -> String {
    if s.chars().count() > 40 {
        format!("{}…", s.chars().take(40).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
}

pub fn sha256(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

pub fn sha1(data: &[u8]) -> String {
    hex::encode(Sha1::digest(data))
}

pub fn md5(data: &[u8]) -> String {
    hex::encode(Md5::digest(data))
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

pub fn crc32(data: &[u8]) -> String {
    format!("{:08x}", crc32fast::hash(data))
}

fn check_key(key: &[u8]) -> Result<(), String> {
    match key.len() {
        16 | 24 | 32 => Ok(()),
        n => Err(format!("AES 密钥长度必须为 16/24/32 字节，当前为 {n} 字节")),
    }
}

/// Returns `iv || ciphertext || tag`.
pub fn aes_gcm_encrypt(plaintext: &[u8], key: &[u8], iv: Option<&[u8]>) -> Result<Vec<u8>, String> {
    check_key(key)?;
    let iv = match iv {
        Some(iv) if iv.len() != GCM_IV_LEN => {
            return Err(format!("AES-GCM IV 必须为 {GCM_IV_LEN} 字节，当前为 {} 字节", iv.len()));
        }
        Some(iv) => iv.to_vec(),
        None => random_bytes(GCM_IV_LEN),
    };
    let nonce = Nonce::from_slice(&iv);
    let ct = match key.len() {
        16 => Aes128Gcm::new_from_slice(key).unwrap().encrypt(nonce, plaintext),
        24 => Aes192Gcm::new_from_slice(key).unwrap().encrypt(nonce, plaintext),
        _ => Aes256Gcm::new_from_slice(key).unwrap().encrypt(nonce, plaintext),
    }
    .map_err(|_| "AES-GCM 加密失败".to_string())?;
    let mut out = iv;
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Expects `iv(12) || ciphertext || tag`.
pub fn aes_gcm_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    check_key(key)?;
    if data.len() < GCM_IV_LEN + 16 {
        return Err("AES-GCM 密文长度不足".into());
    }
    let (iv, ct) = data.split_at(GCM_IV_LEN);
    let nonce = Nonce::from_slice(iv);
    match key.len() {
        16 => Aes128Gcm::new_from_slice(key).unwrap().decrypt(nonce, ct),
        24 => Aes192Gcm::new_from_slice(key).unwrap().decrypt(nonce, ct),
        _ => Aes256Gcm::new_from_slice(key).unwrap().decrypt(nonce, ct),
    }
    .map_err(|_| "AES-GCM 解密失败（密钥错误或数据被篡改）".to_string())
}

/// PKCS#7-padded AES-CBC. Returns `iv || ciphertext`.
pub fn aes_cbc_encrypt(plaintext: &[u8], key: &[u8], iv: Option<&[u8]>) -> Result<Vec<u8>, String> {
    check_key(key)?;
    let iv = match iv {
        Some(iv) if iv.len() != CBC_IV_LEN => {
            return Err(format!("AES-CBC IV 必须为 {CBC_IV_LEN} 字节，当前为 {} 字节", iv.len()));
        }
        Some(iv) => iv.to_vec(),
        None => random_bytes(CBC_IV_LEN),
    };
    let ct = match key.len() {
        16 => cbc::Encryptor::<aes::Aes128>::new_from_slices(key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext),
        24 => cbc::Encryptor::<aes::Aes192>::new_from_slices(key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext),
        _ => cbc::Encryptor::<aes::Aes256>::new_from_slices(key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext),
    };
    let mut out = iv;
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Expects `iv(16) || ciphertext`.
pub fn aes_cbc_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    check_key(key)?;
    if data.len() < CBC_IV_LEN + 16 || !(data.len() - CBC_IV_LEN).is_multiple_of(16) {
        return Err("AES-CBC 密文长度无效".into());
    }
    let (iv, ct) = data.split_at(CBC_IV_LEN);
    let result = match key.len() {
        16 => cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(ct),
        24 => cbc::Decryptor::<aes::Aes192>::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(ct),
        _ => cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(ct),
    };
    result.map_err(|_| "AES-CBC 解密失败（密钥错误或填充无效）".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_known_vectors() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha1(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(md5(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog"),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
        assert_eq!(crc32(b"123456789"), "cbf43926");
    }

    #[test]
    fn aes_roundtrips() {
        for key_len in [16, 24, 32] {
            let key = random_bytes(key_len);
            let gcm = aes_gcm_encrypt(b"hello", &key, None).unwrap();
            assert_eq!(aes_gcm_decrypt(&gcm, &key).unwrap(), b"hello");
            let cbc = aes_cbc_encrypt(b"hello world, 16+", &key, None).unwrap();
            assert_eq!(cbc.len(), 16 + 32); // one full padding block added
            assert_eq!(aes_cbc_decrypt(&cbc, &key).unwrap(), b"hello world, 16+");
        }
        assert!(aes_gcm_encrypt(b"x", &[0u8; 15], None).is_err());
        assert!(aes_gcm_encrypt(b"x", &[0u8; 16], Some(&[0u8; 8])).is_err());
    }

    #[test]
    fn aes_gcm_known_vector() {
        // NIST GCM test case 2 (AES-128, zero key/IV, one zero block).
        let out = aes_gcm_encrypt(&[0u8; 16], &[0u8; 16], Some(&[0u8; 12])).unwrap();
        assert_eq!(
            hex::encode(&out[12..]),
            "0388dace60b6a392f328c2b971b2fe78ab6e47d42cec13bdf53a67b21257bddf"
        );
    }

    #[test]
    fn base64_is_lenient() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGVsbG8").unwrap(), b"hello");
        assert_eq!(base64_decode(" aGVs\nbG8= ").unwrap(), b"hello");
        assert!(base64_decode("***").is_err());
    }
}
