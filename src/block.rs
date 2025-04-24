use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use hex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub timestamp: i64,
    pub prev_hash: String,
    pub merkle_root: String,
    pub nonce: u64,
    pub difficulty: u64,
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
        let max_iterations = 1000000; // 设置一个合理的最大迭代次数
        let mut iterations = 0;
        
        while !self.is_valid() && iterations < max_iterations {
            self.header.nonce += 1;
            iterations += 1;
            
            // 每10000次迭代打印一次进度
            if iterations % 10000 == 0 {
                println!("Mining... iterations: {}, nonce: {}", iterations, self.header.nonce);
            }
        }
        
        if iterations >= max_iterations {
            println!("挖矿达到最大迭代次数限制，未找到满足条件的哈希");
        } else {
            println!("成功挖到区块，迭代次数: {}, nonce: {}", iterations, self.header.nonce);
        }
    }

    pub fn is_valid(&self) -> bool {
        let hash = self.calculate_hash();
        // 检查哈希值前缀是否有足够的0
        // 简单高效的方法：检查哈希值的前n个字符是否都是0
        let prefix_zeros = self.header.difficulty as usize;
        if prefix_zeros == 0 {
            return true; // 如果难度为0，任何哈希值都有效
        }
        
        // 检查哈希值前缀是否有足够的0
        let required_prefix = "0".repeat(prefix_zeros);
        hash.starts_with(&required_prefix)
    }
}

impl Transaction {
    pub fn new(inputs: Vec<TxInput>, outputs: Vec<TxOutput>) -> Self {
        Transaction { inputs, outputs }
    }
} 