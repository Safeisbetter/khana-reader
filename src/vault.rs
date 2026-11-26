//! At-rest authenticated encryption (HMAC-SHA256-CTR + Encrypt-then-MAC) for Khana's private data.
//!
//! When App Lock is enabled, sensitive user files (`settings.json`, `my_voice.wav`, `edits/*.json`)
//! are transparently encrypted on disk using keys derived from the user's App Lock password with a
//! separate PBKDF2 domain salt. Keys live only in RAM while unlocked and are zeroized on lock.

use crate::ffi::wipe;
use crate::lock;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAGIC: &[u8; 12] = b"KHANA-ENC-V1";
const NONCE_LEN: usize = 16;
const TAG_LEN: usize = 32;
const HEADER_LEN: usize = MAGIC.len() + NONCE_LEN;
const MAX_VAULT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone)]
pub struct VaultKey {
    enc: [u8; 32],
    mac: [u8; 32],
}

impl Drop for VaultKey {
    fn drop(&mut self) {
        wipe::bytes(&mut self.enc);
        wipe::bytes(&mut self.mac);
    }
}

static ACTIVE_KEY: Mutex<Option<VaultKey>> = Mutex::new(None);

pub fn derive_key(pw: &str, lock_salt: &[u8], iter: u32) -> VaultKey {
    let mut norm = lock::normalize(pw);
    let mut domain_salt = Vec::with_capacity(lock_salt.len() + 14);
    domain_salt.extend_from_slice(lock_salt);
    domain_salt.extend_from_slice(b"khana-vault-v1");
    let mut km = [0u8; 64];
    lock::pbkdf2_sha256(norm.as_bytes(), &domain_salt, iter, &mut km);
    wipe::string(&mut norm);
    wipe::bytes(&mut domain_salt);
    let mut enc = [0u8; 32];
    let mut mac = [0u8; 32];
    enc.copy_from_slice(&km[..32]);
    mac.copy_from_slice(&km[32..]);
    wipe::bytes(&mut km);
    VaultKey { enc, mac }
}

pub fn install_key(key: VaultKey) {
    let mut guard = ACTIVE_KEY.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(key);
}

pub fn take_key() -> Option<VaultKey> {
    let mut guard = ACTIVE_KEY.lock().unwrap_or_else(|e| e.into_inner());
    guard.take()
}

pub fn clear_key() {
    let _ = take_key();
}

pub fn is_encrypted(data: &[u8]) -> bool {
    data.len() >= HEADER_LEN + TAG_LEN && &data[..MAGIC.len()] == MAGIC
}

fn ctr_xor(enc_key: &[u8; 32], nonce: &[u8; NONCE_LEN], data: &mut [u8]) {
    let mut counter: u32 = 1;
    let mut input = [0u8; NONCE_LEN + 4];
    input[..NONCE_LEN].copy_from_slice(nonce);
    for chunk in data.chunks_mut(32) {
        input[NONCE_LEN..].copy_from_slice(&counter.to_be_bytes());
        let mut ks = lock::hmac_sha256(enc_key, &input);
        for (b, k) in chunk.iter_mut().zip(ks.iter()) {
            *b ^= *k;
        }
        wipe::bytes(&mut ks);
        counter = counter.wrapping_add(1);
    }
    wipe::bytes(&mut input);
}

pub fn encrypt_with_key(key: &VaultKey, plaintext: &[u8]) -> io::Result<Vec<u8>> {
    if plaintext.len() > MAX_VAULT_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "vault payload too large"));
    }
    let nonce = crate::app::random_bytes::<NONCE_LEN>();
    let mut out = Vec::with_capacity(HEADER_LEN + plaintext.len() + TAG_LEN);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(plaintext);
    ctr_xor(&key.enc, &nonce, &mut out[HEADER_LEN..]);
    let tag = lock::hmac_sha256(&key.mac, &out);
    out.extend_from_slice(&tag);
    Ok(out)
}

pub fn decrypt_with_key(key: &VaultKey, blob: &[u8]) -> Option<Vec<u8>> {
    if !is_encrypted(blob) || blob.len() > MAX_VAULT_BYTES + HEADER_LEN + TAG_LEN {
        return None;
    }
    let body_end = blob.len() - TAG_LEN;
    let want_tag = &blob[body_end..];
    let mut got_tag = lock::hmac_sha256(&key.mac, &blob[..body_end]);
    let ok = lock::same_bytes(&got_tag, want_tag);
    wipe::bytes(&mut got_tag);
    if !ok {
        return None;
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&blob[MAGIC.len()..HEADER_LEN]);
    let mut plain = blob[HEADER_LEN..body_end].to_vec();
    ctr_xor(&key.enc, &nonce, &mut plain);
    Some(plain)
}

/// Reads a file from disk, transparently decrypting it if it carries the vault header.
pub fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let raw = std::fs::read(path)?;
    if !is_encrypted(&raw) {
        return Ok(raw);
    }
    let guard = ACTIVE_KEY.lock().unwrap_or_else(|e| e.into_inner());
    let Some(key) = guard.as_ref() else {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "vault is locked"));
    };
    decrypt_with_key(key, &raw)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "vault authentication failed"))
}

/// Writes a file atomically (`0o600`), encrypting it automatically when the vault key is active.
pub fn write_bytes(path: &Path, plaintext: &[u8]) -> io::Result<()> {
    let maybe_key = ACTIVE_KEY.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(key) = maybe_key {
        let mut enc = encrypt_with_key(&key, plaintext)?;
        let res = crate::private_fs::atomic_write(path, &enc);
        wipe::bytes(&mut enc);
        res
    } else {
        crate::private_fs::atomic_write(path, plaintext)
    }
}

fn vault_targets(data_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in ["settings.json", "my_voice.wav"] {
        let p = data_dir.join(name);
        if p.is_file() {
            files.push(p);
        }
    }
    if let Ok(rd) = std::fs::read_dir(data_dir.join("edits")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().is_some_and(|x| x == "json") {
                files.push(p);
            }
        }
    }
    files
}

/// Encrypts any plaintext private files in `data_dir` using `new_key`, or re-encrypts
/// files previously encrypted with `old_key`.
pub fn migrate_to_key(data_dir: &Path, old_key: Option<&VaultKey>, new_key: &VaultKey) {
    for path in vault_targets(data_dir) {
        let Ok(mut raw) = std::fs::read(&path) else { continue };
        let plain_opt = if is_encrypted(&raw) {
            old_key.and_then(|k| decrypt_with_key(k, &raw))
        } else {
            Some(raw.clone())
        };
        wipe::bytes(&mut raw);
        let Some(mut plain) = plain_opt else { continue };
        if let Ok(mut enc) = encrypt_with_key(new_key, &plain) {
            let _ = crate::private_fs::atomic_write(&path, &enc);
            wipe::bytes(&mut enc);
        }
        wipe::bytes(&mut plain);
    }
}

/// Decrypts all vault-encrypted private files in `data_dir` back to plaintext (used when disabling App Lock).
pub fn migrate_to_plain(data_dir: &Path, old_key: &VaultKey) {
    for path in vault_targets(data_dir) {
        let Ok(mut raw) = std::fs::read(&path) else { continue };
        if !is_encrypted(&raw) {
            wipe::bytes(&mut raw);
            continue;
        }
        let plain_opt = decrypt_with_key(old_key, &raw);
        wipe::bytes(&mut raw);
        let Some(mut plain) = plain_opt else { continue };
        let _ = crate::private_fs::atomic_write(&path, &plain);
        wipe::bytes(&mut plain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper_detection() {
        let key = derive_key("رمز-آزمایشی-۱۲۳۴", b"0123456789abcdef", 1000);
        let msg = b"Persian secret book paragraph \xd8\xb3\xd9\x84\xd8\xa7\xd9\x85";
        let mut ct = encrypt_with_key(&key, msg).unwrap();
        assert!(is_encrypted(&ct));
        assert_ne!(&ct[HEADER_LEN..ct.len() - TAG_LEN], msg);
        assert_eq!(decrypt_with_key(&key, &ct).unwrap(), msg);

        // Flip 1 bit in ciphertext -> must reject
        ct[HEADER_LEN] ^= 1;
        assert!(decrypt_with_key(&key, &ct).is_none());

        // Wrong key -> must reject
        ct[HEADER_LEN] ^= 1;
        let other = derive_key("wrong-password", b"0123456789abcdef", 1000);
        assert!(decrypt_with_key(&other, &ct).is_none());
    }
}
