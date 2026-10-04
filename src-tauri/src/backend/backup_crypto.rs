use super::{invalid, Result};
use ring::{
    aead, pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use std::num::NonZeroU32;
const MAGIC: &[u8; 8] = b"GMBKENC1";
fn key(password: &str, salt: &[u8]) -> Result<aead::LessSafeKey> {
    if password.len() < 8 || password.len() > 1024 {
        return Err(invalid("备份密码需要 8～1024 字节。"));
    }
    let mut bytes = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(600_000).unwrap(),
        salt,
        password.as_bytes(),
        &mut bytes,
    );
    let result = aead::UnboundKey::new(&aead::AES_256_GCM, &bytes)
        .map(aead::LessSafeKey::new)
        .map_err(|_| invalid("加密初始化失败。"));
    bytes.fill(0);
    result
}
pub fn encrypted(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
pub fn seal(mut bytes: Vec<u8>, password: &str) -> Result<Vec<u8>> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    let random = SystemRandom::new();
    random
        .fill(&mut salt)
        .and_then(|_| random.fill(&mut nonce))
        .map_err(|_| invalid("无法生成加密随机值。"))?;
    key(password, &salt)?
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(MAGIC.as_slice()),
            &mut bytes,
        )
        .map_err(|_| invalid("加密失败。"))?;
    let mut out = MAGIC.to_vec();
    out.extend(salt);
    out.extend(nonce);
    out.extend(bytes);
    Ok(out)
}
pub fn open(bytes: &[u8], password: &str) -> Result<Vec<u8>> {
    if !encrypted(bytes) {
        return Ok(bytes.to_vec());
    }
    if bytes.len() < 52 {
        return Err(invalid("加密包已损坏。"));
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes[24..36]);
    let mut data = bytes[36..].to_vec();
    let plain = key(password, &bytes[8..24])?
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(MAGIC.as_slice()),
            &mut data,
        )
        .map_err(|_| invalid("密码不正确或备份包已损坏。"))?;
    Ok(plain.to_vec())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_packages_reject_wrong_password_and_tampering() {
        let original = b"private account credentials";
        let mut encrypted = seal(original.to_vec(), "fixture-password").unwrap();
        assert!(!encrypted.windows(original.len()).any(|w| w == original));
        assert_eq!(open(&encrypted, "fixture-password").unwrap(), original);
        assert!(open(&encrypted, "wrong-password").is_err());
        *encrypted.last_mut().unwrap() ^= 1;
        assert!(open(&encrypted, "fixture-password").is_err());
    }
}
