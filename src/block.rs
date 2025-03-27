use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use hex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub timestamp: i64,
    pub prev_hash: String,
    pub merkle_root: String,
    pub nonce: u64,
    pub difficulty: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub prev_tx: String,
    pub prev_index: u32,
    pub script_sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u64,
    pub script_pubkey: String,
}

impl Block {
    pub fn new(prev_hash: String, difficulty: u64) -> Self {
        Block {
            header: BlockHeader {
                timestamp: Utc::now().timestamp(),
                prev_hash,
                merkle_root: String::new(),
                nonce: 0,
                difficulty,
            },
            transactions: Vec::new(),
        }
    }

    pub fn calculate_hash(&self) -> String {
        let mut hasher = sha2::Sha256::new();
        let serialized = serde_json::to_string(&self).unwrap();
        hasher.update(serialized.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn mine(&mut self) {
        while !self.is_valid() {
            self.header.nonce += 1;
        }
    }

    pub fn is_valid(&self) -> bool {
        let hash = self.calculate_hash();
        let target = 2u64.pow(256 - self.header.difficulty as u32);
        let hash_value = u64::from_str_radix(&hash[..16], 16).unwrap();
        hash_value < target
    }
}

impl Transaction {
    pub fn new(inputs: Vec<TxInput>, outputs: Vec<TxOutput>) -> Self {
        Transaction { inputs, outputs }
    }
} 