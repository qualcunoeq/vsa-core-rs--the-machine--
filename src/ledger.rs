use crate::Hypervector;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

type HmacSha256 = Hmac<Sha256>;

pub struct LongTermLedger {
    secret_key: Vec<u8>,
    file_path: String,
}

impl LongTermLedger {
    pub fn new(api_key: &str, file_path: &str) -> Self {
        LongTermLedger {
            secret_key: api_key.as_bytes().to_vec(),
            file_path: file_path.to_string(),
        }
    }

    /// Derives an encryption key from the secret key, state vector, and salt using PBKDF2-HMAC-SHA256
    pub fn derive_key(&self, state_vector: &Hypervector, salt: &[u8]) -> Vec<u8> {
        let mut password = self.secret_key.clone();
        password.extend_from_slice(&state_vector.to_bytes());
        let rounds = 1000;
        let key_len = 32; // AES-256 equivalent key size

        let mut result = Vec::with_capacity(key_len);
        let mut block_num = 1u32;

        while result.len() < key_len {
            let mut hmac = HmacSha256::new_from_slice(&password).unwrap();
            hmac.update(salt);
            hmac.update(&block_num.to_be_bytes());
            let mut u = hmac.finalize().into_bytes();
            let mut xor_sum = u.clone();

            for _ in 1..rounds {
                let mut hmac = HmacSha256::new_from_slice(&password).unwrap();
                hmac.update(&u);
                u = hmac.finalize().into_bytes();
                for (a, b) in xor_sum.iter_mut().zip(u.iter()) {
                    *a ^= b;
                }
            }

            let take = std::cmp::min(key_len - result.len(), xor_sum.len());
            result.extend_from_slice(&xor_sum[..take]);
            block_num += 1;
        }
        result
    }

    /// Encryption/decryption using the custom HMAC-SHA256 Counter (CTR) stream cipher
    pub fn encrypt_decrypt_xor(&self, data: &[u8], key: &[u8]) -> Vec<u8> {
        let mut keystream = Vec::new();
        let mut counter = 0u32;
        while keystream.len() < data.len() {
            let mut hmac = HmacSha256::new_from_slice(key).unwrap();
            hmac.update(&counter.to_be_bytes());
            let block = hmac.finalize().into_bytes();
            keystream.extend_from_slice(&block);
            counter += 1;
        }
        data.iter()
            .zip(keystream.iter())
            .map(|(a, b)| a ^ b)
            .collect()
    }

    /// Appends a new hypervector record to the binary ledger
    /// Record size: Date (10 bytes) + Salt (16 bytes) + Ciphertext (1254 bytes) = 1280 bytes
    pub fn append_record(
        &self,
        date_str: &str,
        vector: &Hypervector,
        state_vector: &Hypervector,
    ) -> Result<(), String> {
        if date_str.len() != 10 {
            return Err("Date string must be exactly 10 characters (YYYY-MM-DD)".to_string());
        }

        // 1. Serialize vector to 1250 bytes and prepend 4 magic bytes
        let raw_bytes = vector.to_bytes_1250();
        let mut payload = vec![0u8; 1254];
        payload[0..4].copy_from_slice(&[0xDE, 0xAD, 0xC0, 0xDE]);
        payload[4..1254].copy_from_slice(&raw_bytes);

        // 2. Generate random salt (16 bytes)
        let mut rng = rand::thread_rng();
        let mut salt = [0u8; 16];
        rng.fill(&mut salt);

        // 3. Encrypt the payload bytes
        let derived_key = self.derive_key(state_vector, &salt);
        let ciphertext = self.encrypt_decrypt_xor(&payload, &derived_key);

        // 4. Assemble the 1280-byte record
        let mut record = Vec::with_capacity(1280);
        record.extend_from_slice(date_str.as_bytes());
        record.extend_from_slice(&salt);
        record.extend_from_slice(&ciphertext);

        // 5. Append to file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map_err(|e| format!("Failed to open ledger file: {}", e))?;

        file.write_all(&record)
            .map_err(|e| format!("Failed to write record: {}", e))?;

        Ok(())
    }

    /// Reads and decrypts all records from the binary ledger file using the specified state_vector.
    /// Returns Err("DECRYPTION_FAILED_SECURITY_LOCK") if the key is incorrect.
    pub fn load_records(
        &self,
        state_vector: &Hypervector,
    ) -> Result<Vec<(String, Hypervector)>, String> {
        let path = Path::new(&self.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(path).map_err(|e| format!("Failed to open ledger: {}", e))?;

        let mut results = Vec::new();
        let record_len = 1280;
        let mut buffer = vec![0u8; record_len];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read ledger: {}", e))?;

            if bytes_read == 0 {
                break;
            }
            if bytes_read < record_len {
                break;
            }

            let date_bytes = &buffer[0..10];
            let date_str = String::from_utf8_lossy(date_bytes).into_owned();

            let salt = &buffer[10..26];
            let ciphertext = &buffer[26..1280];

            let derived_key = self.derive_key(state_vector, salt);
            let decrypted_payload = self.encrypt_decrypt_xor(ciphertext, &derived_key);

            // Verify Magic Signature Check
            if decrypted_payload[0..4] != [0xDE, 0xAD, 0xC0, 0xDE] {
                return Err("DECRYPTION_FAILED_SECURITY_LOCK".to_string());
            }

            let mut hv_bytes = [0u8; 1250];
            hv_bytes.copy_from_slice(&decrypted_payload[4..1254]);
            let vector = Hypervector::from_bytes_1250(&hv_bytes);

            results.push((date_str, vector));
        }

        Ok(results)
    }
}

// Add these to Hypervector in lib.rs or locally
impl Hypervector {
    pub fn to_bytes_1250(&self) -> [u8; 1250] {
        let mut bytes = [0u8; 1250];
        for i in 0..156 {
            let block_bytes = self.bits[i].to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&block_bytes);
        }
        let last_bytes = self.bits[156].to_le_bytes();
        bytes[1248] = last_bytes[0];
        bytes[1249] = last_bytes[1];
        bytes
    }

    pub fn from_bytes_1250(bytes: &[u8; 1250]) -> Self {
        let mut bits = [0u64; 157];
        for i in 0..156 {
            let mut block_bytes = [0u8; 8];
            block_bytes.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            bits[i] = u64::from_le_bytes(block_bytes);
        }
        let mut last_bytes = [0u8; 8];
        last_bytes[0] = bytes[1248];
        last_bytes[1] = bytes[1249];
        bits[156] = u64::from_le_bytes(last_bytes);
        Hypervector { bits }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_state_based_encryption() {
        let temp_file = "data/temp_test_ledger.bin";
        let static_key = "test_finch_key";
        let ledger = LongTermLedger::new(static_key, temp_file);

        let test_state = Hypervector::new_random();
        let wrong_state = Hypervector::new_random();
        let record_vector = Hypervector::new_random();

        // Append record using test_state
        let append_res = ledger.append_record("2026-06-04", &record_vector, &test_state);
        assert!(append_res.is_ok());

        // Try to load with the correct state vector
        let load_res = ledger.load_records(&test_state);
        assert!(load_res.is_ok());
        let records = load_res.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, "2026-06-04");

        let dist = records[0].1.normalized_hamming_distance(&record_vector);
        assert!(dist < 0.01);

        // Try to load with wrong state vector -> should fail with DECRYPTION_FAILED_SECURITY_LOCK error
        let load_wrong_res = ledger.load_records(&wrong_state);
        assert!(load_wrong_res.is_err());
        assert_eq!(
            load_wrong_res.unwrap_err(),
            "DECRYPTION_FAILED_SECURITY_LOCK"
        );

        // Clean up temp file
        let _ = fs::remove_file(temp_file);
    }
}
