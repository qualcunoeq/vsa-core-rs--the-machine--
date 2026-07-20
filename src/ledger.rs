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
        let key_len = 32;

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

    /// ██ UPGRADE v2.0: 1280-byte payload for D=10240 ██
    ///
    /// Record size: Date (10 bytes) + Salt (16 bytes) + Ciphertext (1284 bytes) = 1310 bytes
    /// Ciphertext = Magic (4 bytes) + Payload (1280 bytes = 160 × u64)
    pub fn append_record(
        &self,
        date_str: &str,
        vector: &Hypervector,
        state_vector: &Hypervector,
    ) -> Result<(), String> {
        if date_str.len() != 10 {
            return Err("Date string must be exactly 10 characters (YYYY-MM-DD)".to_string());
        }

        // 1. Serialize vector to 1280 bytes and prepend 4 magic bytes
        let raw_bytes = vector.to_bytes();
        let mut payload = vec![0u8; 1284];
        payload[0..4].copy_from_slice(&[0xDE, 0xAD, 0xC0, 0xDE]);
        payload[4..1284].copy_from_slice(&raw_bytes);

        // 2. Generate random salt (16 bytes)
        let mut rng = rand::thread_rng();
        let mut salt = [0u8; 16];
        rng.fill(&mut salt);

        // 3. Encrypt the payload bytes
        let derived_key = self.derive_key(state_vector, &salt);
        let ciphertext = self.encrypt_decrypt_xor(&payload, &derived_key);

        // 4. Assemble the 1310-byte record
        let mut record = Vec::with_capacity(1310);
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
        let record_len = 1310;
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
            let ciphertext = &buffer[26..1310];

            let derived_key = self.derive_key(state_vector, salt);
            let decrypted_payload = self.encrypt_decrypt_xor(ciphertext, &derived_key);

            // Verify Magic Signature Check
            if decrypted_payload[0..4] != [0xDE, 0xAD, 0xC0, 0xDE] {
                return Err("DECRYPTION_FAILED_SECURITY_LOCK".to_string());
            }

            let mut hv_bytes = [0u8; 1280];
            hv_bytes.copy_from_slice(&decrypted_payload[4..1284]);
            let vector = Hypervector::from_bytes(&hv_bytes);

            results.push((date_str, vector));
        }

        Ok(results)
    }

    // ─── Ledger Compaction Protocol (The "Sleep Cycle") ──────────────────

    /// Agglomerative clustering + majority-rule re-bundling.
    ///
    /// 1. Loads all decrypted records.
    /// 2. Groups vectors whose pairwise similarity ≥ `SIMILARITY_THRESHOLD`.
    /// 3. Collapses each cluster into a single centroid via bit-parallel
    ///    majority bundling.
    /// 4. Rewrites the binary file with only the compacted centroids.
    ///
    /// Returns the number of records *removed* (original − compacted),
    /// or an error if decryption fails or the file cannot be rewritten.
    pub fn compact_ledger(
        &self,
        state_vector: &Hypervector,
        similarity_threshold: f64,
    ) -> Result<usize, String> {
        // 1. Load all decrypted records
        let records = self.load_records(state_vector)?;
        let original_count = records.len();
        if original_count == 0 {
            return Ok(0);
        }

        // 2. Agglomerative clustering (greedy single-link)
        let mut clusters: Vec<Vec<Hypervector>> = Vec::new();

        for (_, vector) in &records {
            let mut assigned = false;
            for cluster in &mut clusters {
                let centroid = &cluster[0];
                let sim = 1.0 - vector.normalized_hamming_distance(centroid);
                if sim >= similarity_threshold {
                    cluster.push(vector.clone());
                    assigned = true;
                    break;
                }
            }
            if !assigned {
                clusters.push(vec![vector.clone()]);
            }
        }

        // 3. Majority-rule re-bundling within each cluster
        let compacted: Vec<Hypervector> = clusters
            .iter()
            .map(|cluster| {
                if cluster.len() <= 1 {
                    return cluster[0];
                }
                let refs: Vec<&Hypervector> = cluster.iter().collect();
                Hypervector::bundle(&refs)
            })
            .collect();

        // 4. Cryptographic rewrite
        let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let path = std::path::Path::new(&self.file_path);
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|e| format!("Failed to remove old ledger: {}", e))?;
        }

        for vector in &compacted {
            self.append_record(&today_str, vector, state_vector)?;
        }

        let removed = original_count - compacted.len();
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_state_based_encryption() {
        std::fs::create_dir_all("data").unwrap();
        let temp_file = "data/temp_test_ledger.bin";
        let static_key = "test_finch_key";
        let ledger = LongTermLedger::new(static_key, temp_file);

        let test_state = Hypervector::new_random();
        let wrong_state = Hypervector::new_random();
        let record_vector = Hypervector::new_random();

        let append_res = ledger.append_record("2026-06-04", &record_vector, &test_state);
        assert!(append_res.is_ok());

        let load_res = ledger.load_records(&test_state);
        assert!(load_res.is_ok());
        let records = load_res.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, "2026-06-04");

        let dist = records[0].1.normalized_hamming_distance(&record_vector);
        assert!(dist < 0.01);

        let load_wrong_res = ledger.load_records(&wrong_state);
        assert!(load_wrong_res.is_err());
        assert_eq!(
            load_wrong_res.unwrap_err(),
            "DECRYPTION_FAILED_SECURITY_LOCK"
        );

        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_ledger_compaction() {
        std::fs::create_dir_all("data").unwrap();
        let temp_file = "data/temp_test_compaction.bin";
        let _ = std::fs::remove_file(temp_file);
        let static_key = "test_compaction_key";
        let ledger = LongTermLedger::new(static_key, temp_file);
        let state = Hypervector::new_random();

        let v1 = Hypervector::new_random();
        let v2 = Hypervector::new_random();
        let v3 = Hypervector::new_random();
        let v4 = Hypervector::new_random();
        let v5 = Hypervector::new_random();
        ledger.append_record("2026-06-01", &v1, &state).unwrap();
        ledger.append_record("2026-06-02", &v2, &state).unwrap();
        ledger.append_record("2026-06-03", &v3, &state).unwrap();
        ledger.append_record("2026-06-04", &v4, &state).unwrap();
        ledger.append_record("2026-06-05", &v5, &state).unwrap();
        assert_eq!(ledger.load_records(&state).unwrap().len(), 5);

        let removed = ledger.compact_ledger(&state, 0.99).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(ledger.load_records(&state).unwrap().len(), 5);

        ledger.append_record("2026-06-06", &v1, &state).unwrap();
        assert_eq!(ledger.load_records(&state).unwrap().len(), 6);

        let removed = ledger.compact_ledger(&state, 0.70).unwrap();
        assert!(
            removed >= 1,
            "Should have merged the duplicate: removed={}",
            removed
        );
        assert_eq!(ledger.load_records(&state).unwrap().len(), 5);

        let _ = std::fs::remove_file(temp_file);
    }
}
