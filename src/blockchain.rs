use std::collections::HashMap;
use crate::block::{Block, Transaction};
use std::fs;
use std::path::Path;
use sha2::{Sha256, Digest};

pub struct Blockchain {
    pub blocks: Vec<Block>,
    pub utxo_set: HashMap<String, Vec<(u32, u64)>>, // tx_id -> [(output_index, amount)]
    pub difficulty: u64,
}

impl Blockchain {
    pub fn new(difficulty: u64) -> Self {
        let genesis_block = Block::new(String::from("0"), difficulty);
        let blockchain = Blockchain {
            blocks: vec![genesis_block],
            utxo_set: HashMap::new(),
            difficulty,
        };
        blockchain.save_to_file("blockchain.json");
        blockchain
    }

    pub fn add_block(&mut self, transactions: Vec<Transaction>) {
        let prev_block = self.blocks.last().unwrap();
        let prev_hash = prev_block.calculate_hash();
        
        let mut new_block = Block::new(prev_hash, self.difficulty);
        new_block.transactions = transactions;
        new_block.mine();
        
        self.blocks.push(new_block);
        self.update_utxo_set();
        self.save_to_file("blockchain.json");
    }

    fn update_utxo_set(&mut self) {
        self.utxo_set.clear();
        
        for block in &self.blocks {
            for tx in &block.transactions {
                let tx_id = self.calculate_tx_hash(tx);
                
                // 处理输出
                for (index, output) in tx.outputs.iter().enumerate() {
                    let outputs = self.utxo_set.entry(tx_id.clone())
                        .or_insert_with(Vec::new);
                    outputs.push((index as u32, output.value));
                }
                
                // 处理输入
                for input in &tx.inputs {
                    if let Some(outputs) = self.utxo_set.get_mut(&input.prev_tx) {
                        outputs.retain(|&(idx, _)| idx != input.prev_index);
                    }
                }
            }
        }
    }

    pub fn calculate_tx_hash(&self, tx: &Transaction) -> String {
        let mut hasher = Sha256::new();
        let serialized = serde_json::to_string(tx).unwrap();
        hasher.update(serialized.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn save_to_file(&self, filename: &str) {
        let serialized = serde_json::to_string_pretty(&self.blocks).unwrap();
        fs::write(filename, serialized).expect("Unable to write blockchain to file");
    }

    pub fn load_from_file(filename: &str) -> Option<Self> {
        if !Path::new(filename).exists() {
            return None;
        }

        let contents = fs::read_to_string(filename).ok()?;
        let blocks: Vec<Block> = serde_json::from_str(&contents).ok()?;
        
        let difficulty = blocks[0].header.difficulty;
        let mut blockchain = Blockchain {
            blocks,
            utxo_set: HashMap::new(),
            difficulty,
        };
        
        blockchain.update_utxo_set();
        Some(blockchain)
    }

    pub fn get_balance(&self, _address: &str) -> u64 {
        let mut balance = 0;
        for (_, outputs) in &self.utxo_set {
            for (_, amount) in outputs {
                balance += amount;
            }
        }
        balance
    }
}
