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
#[derive(Clone)]
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
    /// 创建新的区块链实例
    ///
    /// # 参数
    ///
    /// * `difficulty` - 挖矿难度，影响新区块的哈希要求
    ///
    /// # 返回值
    ///
    /// 返回初始化的区块链实例，包含创世区块
    pub fn new(difficulty: u64) -> Self {
        let mut blockchain = Blockchain {
            blocks: Vec::new(),
            utxo_set: HashMap::new(),
            difficulty,
        };
        
        // 创建固定的创世区块，确保所有节点一致
        blockchain.create_genesis_block();
        blockchain.update_utxo_set();
        blockchain
    }
    
    /// 创建固定的创世区块
    fn create_genesis_block(&mut self) {
        // 使用固定的时间戳和数据，确保所有节点的创世区块相同
        let genesis_header = crate::block::BlockHeader {
            prev_hash: String::from("0"),
            timestamp: 1748793600, // 固定时间戳：2025-06-01 00:00:00
            merkle_root: String::from("genesis_merkle_root"), // 固定的默克尔根
            nonce: 0,
            difficulty: self.difficulty,
        };
        
        // 创世区块包含一个固定的coinbase交易
        let genesis_coinbase = crate::block::Transaction::new(
            vec![crate::block::TxInput {
                prev_tx: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
                prev_index: 0,
                script_sig: String::from("Genesis Block - Blockchain Demo"),
            }],
            vec![crate::block::TxOutput {
                value: 100, // 创世区块奖励
                script_pubkey: String::from("genesis_address"), // 固定的创世地址
            }]
        );
        
        let genesis_block = crate::block::Block {
            header: genesis_header,
            transactions: vec![genesis_coinbase],
        };
        
        self.blocks.push(genesis_block);
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
        
        // 首先添加所有交易的输出
        for block in &self.blocks {
            for tx in &block.transactions {
                let tx_id = self.calculate_tx_hash(tx);
                
                // 添加所有输出到UTXO集
                for (index, output) in tx.outputs.iter().enumerate() {
                    let outputs = self.utxo_set.entry(tx_id.clone())
                        .or_insert_with(Vec::new);
                    outputs.push((index as u32, output.value));
                }
            }
        }
        
        // 然后移除所有被花费的输出
        for block in &self.blocks {
            for tx in &block.transactions {
                // 处理输入，移除已花费的UTXO
                for input in &tx.inputs {
                    // 跳过coinbase交易的输入
                    if input.prev_tx == "0000000000000000000000000000000000000000000000000000000000000000" {
                        continue;
                    }
                    
                    // 从UTXO集中移除已花费的输出
                    if let Some(outputs) = self.utxo_set.get_mut(&input.prev_tx) {
                        outputs.retain(|&(idx, _)| idx != input.prev_index);
                        // 如果这个交易的所有输出都被花费了，移除整个条目
                        if outputs.is_empty() {
                            self.utxo_set.remove(&input.prev_tx);
                        }
                    }
                }
            }
        }
        
        // 清理空的条目
        self.utxo_set.retain(|_, outputs| !outputs.is_empty());
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
    /// * `address` - 要查询余额的地址
    ///
    /// # 返回值
    ///
    /// 返回指定地址的余额
    pub fn get_balance(&self, address: &str) -> u64 {
        let mut balance = 0;
        
        // 直接从UTXO集中计算余额，无需遍历所有区块
        for (tx_id, outputs) in &self.utxo_set {
            // 找到对应的交易来获取输出详情
            let mut tx_found = None;
            'outer: for block in &self.blocks {
                for tx in &block.transactions {
                    if self.calculate_tx_hash(tx) == *tx_id {
                        tx_found = Some(tx);
                        break 'outer;
                    }
                }
            }
            
            if let Some(tx) = tx_found {
                // 检查UTXO集中的每个输出
                for &(output_idx, _amount) in outputs {
                    if let Some(output) = tx.outputs.get(output_idx as usize) {
                        if output.script_pubkey == address {
                            balance += output.value;
                        }
                    }
                }
            }
        }
        
        balance
    }

    /// 验证区块是否有效
    ///
    /// # 参数
    ///
    /// * `block` - 要验证的区块
    ///
    /// # 返回值
    ///
    /// 如果区块有效返回true，否则返回false
    pub fn validate_block(&self, block: &Block) -> bool {
        // 1. 验证区块哈希满足难度要求
        if !block.is_valid() {
            println!("区块哈希不满足难度要求");
            return false;
        }

        // 2. 验证前一个区块哈希是否匹配
        if let Some(prev_block) = self.blocks.last() {
            let prev_hash = prev_block.calculate_hash();
            if block.header.prev_hash != prev_hash {
                println!("区块前一个哈希不匹配");
                return false;
            }
        } else if block.header.prev_hash != "0" {
            // 如果是创世区块，前一个哈希应该是0
            println!("创世区块前一个哈希应该是0");
            return false;
        }

        // 3. 验证所有交易
        for tx in &block.transactions {
            if !self.validate_transaction(tx) {
                return false;
            }
        }

        true
    }

    /// 验证交易是否有效
    ///
    /// # 参数
    ///
    /// * `transaction` - 要验证的交易
    ///
    /// # 返回值
    ///
    /// 如果交易有效返回true，否则返回false
    pub fn validate_transaction(&self, transaction: &Transaction) -> bool {
        // 1. 验证交易输入引用的UTXO是否存在
        for input in &transaction.inputs {
            // 对于Coinbase交易跳过验证
            if input.prev_tx == "0000000000000000000000000000000000000000000000000000000000000000" {
                continue;
            }

            // 检查UTXO是否存在
            if let Some(outputs) = self.utxo_set.get(&input.prev_tx) {
                let mut found = false;
                for &(idx, _) in outputs {
                    if idx == input.prev_index {
                        found = true;
                        break;
                    }
                }
                if !found {
                    println!("输入引用的UTXO不存在");
                    return false;
                }
            } else {
                println!("输入引用的交易不在UTXO集中");
                return false;
            }
        }

        // 2. 验证交易签名 (实际实现中应该验证)
        // 简化版暂不验证签名

        // 3. 验证输入总额大于等于输出总额
        // 这需要访问之前的交易，简化版暂不验证

        true
    }

    /// 添加接收到的区块到区块链
    ///
    /// # 参数
    ///
    /// * `block` - 要添加的区块
    pub fn add_received_block(&mut self, block: Block) {
        self.blocks.push(block);
        self.update_utxo_set();
        self.save_to_file("blockchain.json");
    }

    /// 替换本地链
    ///
    /// # 参数
    ///
    /// * `blocks` - 新的区块列表
    pub fn replace_chain(&mut self, blocks: Vec<Block>) {
        self.blocks = blocks;
        self.save_to_file("blockchain.json");
    }

    /// 重建UTXO集
    pub fn rebuild_utxo_set(&mut self) {
        self.update_utxo_set();
    }
    
    /// 调试UTXO集，显示详细信息
    pub fn debug_utxo_set(&self, address: &str) {
        println!("\n=== UTXO集调试信息 ===");
        println!("查询地址: {}", address);
        println!("UTXO集总条目数: {}", self.utxo_set.len());
        
        let mut total_balance = 0;
        for (tx_id, outputs) in &self.utxo_set {
            println!("交易ID: {}", tx_id);
            
            // 找到对应的交易
            let mut tx_found = None;
            'outer: for block in &self.blocks {
                for tx in &block.transactions {
                    if self.calculate_tx_hash(tx) == *tx_id {
                        tx_found = Some(tx);
                        break 'outer;
                    }
                }
            }
            
            if let Some(tx) = tx_found {
                for &(output_idx, amount) in outputs {
                    if let Some(output) = tx.outputs.get(output_idx as usize) {
                        println!("  输出[{}]: {} -> {} (金额: {})", 
                                output_idx, output.script_pubkey, 
                                if output.script_pubkey == address { "✅匹配" } else { "❌不匹配" },
                                output.value);
                        
                        if output.script_pubkey == address {
                            total_balance += output.value;
                        }
                    }
                }
            } else {
                println!("  ⚠️ 找不到对应的交易！");
            }
        }
        
        println!("计算出的余额: {}", total_balance);
        println!("===================\n");
    }
}
