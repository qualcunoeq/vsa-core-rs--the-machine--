use crate::Hypervector;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::fs::{OpenOptions, File};
use std::io::{Read, Write};
use std::path::Path;
use rand::Rng;

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

    /// Derives an encryption key from the secret key and salt using PBKDF2-HMAC-SHA256
    pub fn derive_key(&self, salt: &[u8]) -> Vec<u8> {
        let password = &self.secret_key;
        let rounds = 1000;
        let key_len = 32; // AES-256 equivalent key size
        
        let mut result = Vec::with_capacity(key_len);
        let mut block_num = 1u32;
        
        while result.len() < key_len {
            let mut hmac = HmacSha256::new_from_slice(password).unwrap();
            hmac.update(salt);
            hmac.update(&block_num.to_be_bytes());
            let mut u = hmac.finalize().into_bytes();
            let mut xor_sum = u.clone();
            
            for _ in 1..rounds {
                let mut hmac = HmacSha256::new_from_slice(password).unwrap();
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
        data.iter().zip(keystream.iter()).map(|(a, b)| a ^ b).collect()
    }

    /// Appends a new hypervector record to the binary ledger
    /// Record size: Date (10 bytes) + Salt (16 bytes) + Ciphertext (1250 bytes) = 1276 bytes
    pub fn append_record(&self, date_str: &str, vector: &Hypervector) -> Result<(), String> {
        if date_str.len() != 10 {
            return Err("Date string must be exactly 10 characters (YYYY-MM-DD)".to_string());
        }

        // 1. Serialize vector to 1250 bytes
        let raw_bytes = vector.to_bytes_1250();

        // 2. Generate random salt (16 bytes)
        let mut rng = rand::thread_rng();
        let mut salt = [0u8; 16];
        rng.fill(&mut salt);

        // 3. Encrypt the vector bytes
        let derived_key = self.derive_key(&salt);
        let ciphertext = self.encrypt_decrypt_xor(&raw_bytes, &derived_key);

        // 4. Assemble the 1276-byte record
        let mut record = Vec::with_capacity(1276);
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

    /// Reads and decrypts all records from the binary ledger file
    pub fn load_records(&self) -> Result<Vec<(String, Hypervector)>, String> {
        let path = Path::new(&self.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(path)
            .map_err(|e| format!("Failed to open ledger: {}", e))?;

        let mut results = Vec::new();
        let record_len = 1276;
        let mut buffer = vec![0u8; record_len];

        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| format!("Failed to read ledger: {}", e))?;

            if bytes_read == 0 {
                break;
            }
            if bytes_read < record_len {
                // Incomplete record
                break;
            }

            let date_bytes = &buffer[0..10];
            let date_str = String::from_utf8_lossy(date_bytes).into_owned();

            let salt = &buffer[10..26];
            let ciphertext = &buffer[26..1276];

            let derived_key = self.derive_key(salt);
            let raw_bytes = self.encrypt_decrypt_xor(ciphertext, &derived_key);

            let mut hv_bytes = [0u8; 1250];
            hv_bytes.copy_from_slice(&raw_bytes);
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
            bytes[i * 8 .. (i + 1) * 8].copy_from_slice(&block_bytes);
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
            block_bytes.copy_from_slice(&bytes[i * 8 .. (i + 1) * 8]);
            bits[i] = u64::from_le_bytes(block_bytes);
        }
        let mut last_bytes = [0u8; 8];
        last_bytes[0] = bytes[1248];
        last_bytes[1] = bytes[1249];
        bits[156] = u64::from_le_bytes(last_bytes);
        Hypervector { bits }
    }
}
