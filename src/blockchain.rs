//! # 区块链模块
//! 
//! 实现区块链的核心功能，包括区块链结构、区块添加、UTXO集合管理，以及区块链数据的持久化。
//! 
//! 该模块负责管理区块链的状态，包括维护区块列表和未花费交易输出(UTXO)集合。

use std::collections::HashMap;
use crate::block::{Block, Transaction};
use std::fs;
use std::path::Path;
use sha2::{Sha256, Digest};

/// 区块链结构，包含区块列表、UTXO集合和挖矿难度
pub struct Blockchain {
    /// 区块列表，存储链中所有区块
    pub blocks: Vec<Block>,
    /// UTXO集合，存储未花费的交易输出
    /// 键为交易ID，值为(输出索引, 金额)元组的列表
    pub utxo_set: HashMap<String, Vec<(u32, u64)>>, // tx_id -> [(output_index, amount)]
    /// 挖矿难度，影响新区块的哈希要求
    pub difficulty: u64,
}

impl Blockchain {
    /// 创建新的区块链
    ///
    /// # 参数
    ///
    /// * `difficulty` - 挖矿难度
    ///
    /// # 返回值
    ///
    /// 返回一个带有创世区块的新区块链
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

    /// 向区块链添加新区块
    ///
    /// # 参数
    ///
    /// * `transactions` - 要包含在新区块中的交易列表
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

    /// 更新UTXO集合
    ///
    /// 遍历区块链中的所有交易，重新构建UTXO集合
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

    /// 计算交易哈希值
    ///
    /// # 参数
    ///
    /// * `tx` - 要计算哈希的交易
    ///
    /// # 返回值
    ///
    /// 返回计算得到的交易哈希值（16进制字符串）
    pub fn calculate_tx_hash(&self, tx: &Transaction) -> String {
        let mut hasher = Sha256::new();
        let serialized = serde_json::to_string(tx).unwrap();
        hasher.update(serialized.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// 将区块链数据保存到文件
    ///
    /// # 参数
    ///
    /// * `filename` - 保存区块链数据的文件名
    pub fn save_to_file(&self, filename: &str) {
        let serialized = serde_json::to_string_pretty(&self.blocks).unwrap();
        fs::write(filename, serialized).expect("Unable to write blockchain to file");
    }

    /// 从文件加载区块链数据
    ///
    /// # 参数
    ///
    /// * `filename` - 包含区块链数据的文件名
    ///
    /// # 返回值
    ///
    /// 如果文件存在并且格式正确，返回加载的区块链；否则返回None
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

    /// 获取地址余额
    ///
    /// # 参数
    ///
    /// * `_address` - 要查询余额的地址
    ///
    /// # 返回值
    ///
    /// 返回指定地址的余额
    /// 
    /// # 注意
    /// 
    /// 当前实现计算特定地址的余额
    pub fn get_balance(&self, address: &str) -> u64 {
        let mut balance = 0;
        
        for (tx_id, outputs) in &self.utxo_set {
            for (output_idx, amount) in outputs {
                // 查找此交易
                for block in &self.blocks {
                    for tx in &block.transactions {
                        if self.calculate_tx_hash(tx) == *tx_id {
                            // 检查输出是否属于此地址
                            if let Some(output) = tx.outputs.get(*output_idx as usize) {
                                if output.script_pubkey == address {
                                    balance += amount;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        balance
    }
}
