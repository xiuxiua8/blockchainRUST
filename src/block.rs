//! # 区块模块
//! 
//! 定义区块链中的基本数据结构，包括区块、区块头、交易、交易输入和交易输出。
//! 
//! 该模块是区块链系统的核心部分，实现了区块创建、挖矿和验证等功能。

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use hex;

/// 区块结构，包含区块头和交易列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// 区块头，包含区块元数据
    pub header: BlockHeader,
    /// 区块中包含的交易列表
    pub transactions: Vec<Transaction>,
}

/// 区块头结构，包含区块的元数据信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// 区块创建时间戳
    pub timestamp: i64,
    /// 前一个区块的哈希值
    pub prev_hash: String,
    /// 交易的默克尔根
    pub merkle_root: String,
    /// 工作量证明的随机数
    pub nonce: u64,
    /// 挖矿难度，表示为目标哈希值前导零的数量
    pub difficulty: u64,
}


/// 交易结构，包含交易输入和输出列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// 交易输入列表，表示花费的UTXO
    pub inputs: Vec<TxInput>,
    /// 交易输出列表，表示创建的新UTXO
    pub outputs: Vec<TxOutput>,
}

/// 交易输入结构，引用之前交易的输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    /// 前一个交易的ID
    pub prev_tx: String,
    /// 前一个交易中输出的索引
    pub prev_index: u32,
    /// 脚本签名，用于验证交易
    pub script_sig: String,
}

/// 交易输出结构，表示可花费的金额和接收者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    /// 输出金额
    pub value: u64,
    /// 锁定脚本，通常包含接收者的地址
    pub script_pubkey: String,
}

impl Block {
    /// 创建新的区块
    ///
    /// # 参数
    ///
    /// * `prev_hash` - 前一个区块的哈希值
    /// * `difficulty` - 挖矿难度
    ///
    /// # 返回值
    ///
    /// 返回一个新创建的区块实例
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

    /// 计算区块的哈希值
    ///
    /// # 返回值
    ///
    /// 返回计算得到的区块哈希值（16进制字符串）
    pub fn calculate_hash(&self) -> String {
        let mut hasher = sha2::Sha256::new();
        let serialized = serde_json::to_string(&self).unwrap();
        hasher.update(serialized.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// 挖掘区块，尝试找到满足难度要求的哈希值
    ///
    /// 此方法会调整nonce值，直到找到满足难度要求的哈希值
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

    /// 验证区块是否满足难度要求
    ///
    /// # 返回值
    ///
    /// 如果区块哈希满足难度要求，返回true；否则返回false
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
    /// 创建新的交易
    ///
    /// # 参数
    ///
    /// * `inputs` - 交易输入列表
    /// * `outputs` - 交易输出列表
    ///
    /// # 返回值
    ///
    /// 返回一个新创建的交易实例
    pub fn new(inputs: Vec<TxInput>, outputs: Vec<TxOutput>) -> Self {
        Transaction { inputs, outputs }
    }
} 