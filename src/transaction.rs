use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub tx_id: String,
    pub output_index: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u64,
    pub pub_key_hash: String,
} 