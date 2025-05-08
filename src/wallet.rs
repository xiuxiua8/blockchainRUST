//! # 钱包模块
//! 
//! 实现加密货币钱包功能，包括公私钥管理、地址生成、交易创建和签名等功能。
//! 
//! 该模块使用secp256k1椭圆曲线算法进行密钥生成和交易签名。

use secp256k1::{PublicKey, SecretKey};
use sha2::{Sha256, Digest};
use hex;
use std::collections::HashMap;
use crate::block::{Transaction, TxInput, TxOutput};
use rand;
use serde::{Serialize, Deserialize};
use std::fs;

/// 钱包结构，包含密钥对和地址
#[derive(Serialize, Deserialize)]
pub struct Wallet {
    /// 私钥，用于交易签名
    pub private_key: SecretKey,
    /// 公钥，用于验证签名
    pub public_key: PublicKey,
    /// 钱包地址，公钥的哈希表示
    pub address: String,
}

impl Wallet {
    /// 创建新的钱包
    ///
    /// 生成一个新的密钥对并派生相应的地址
    ///
    /// # 返回值
    ///
    /// 返回一个初始化的钱包实例
    pub fn new() -> Self {
        let secp = secp256k1::Secp256k1::new();
        let mut rng = rand::thread_rng();
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        let address = Self::public_key_to_address(&public_key);
        
        Wallet {
            private_key: secret_key,
            public_key,
            address,
        }
    }

    /// 将公钥转换为钱包地址
    ///
    /// 使用SHA256和RIPEMD160哈希算法对公钥进行双重哈希，然后转换为十六进制字符串
    ///
    /// # 参数
    ///
    /// * `public_key` - 要转换的公钥
    ///
    /// # 返回值
    ///
    /// 返回生成的钱包地址（十六进制字符串）
    fn public_key_to_address(public_key: &PublicKey) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&public_key.serialize_uncompressed());
        let result = hasher.finalize();
        
        // 使用RIPEMD160进行二次哈希
        let mut ripemd = ripemd::Ripemd160::new();
        ripemd.update(&result);
        let result = ripemd.finalize();
        
        hex::encode(result)
    }

    /// 创建新的交易
    ///
    /// # 参数
    ///
    /// * `to_address` - 接收者的地址
    /// * `amount` - 要发送的金额
    /// * `utxo_set` - 当前UTXO集合
    ///
    /// # 返回值
    ///
    /// 如果有足够的UTXO余额，返回创建的交易；否则返回None
    pub fn create_transaction(
        &self,
        to_address: &str,
        amount: u64,
        utxo_set: &HashMap<String, Vec<(u32, u64)>>,
    ) -> Option<Transaction> {
        let mut inputs = Vec::new();
        let mut total_input = 0u64;
        
        // 查找可用的UTXO
        for (tx_id, outputs) in utxo_set {
            for (index, value) in outputs {
                if total_input >= amount {
                    break;
                }
                
                inputs.push(TxInput {
                    prev_tx: tx_id.clone(),
                    prev_index: *index,
                    script_sig: self.address.clone(),
                });
                
                total_input += value;
            }
        }
        
        if total_input < amount {
            return None;
        }
        
        // 创建输出
        let mut outputs = vec![
            TxOutput {
                value: amount,
                script_pubkey: to_address.to_string(),
            },
        ];
        
        // 添加找零输出
        if total_input > amount {
            outputs.push(TxOutput {
                value: total_input - amount,
                script_pubkey: self.address.clone(),
            });
        }
        
        Some(Transaction::new(inputs, outputs))
    }

    /// 签名交易
    ///
    /// 使用钱包的私钥对交易进行签名，使其能被区块链网络验证
    ///
    /// # 参数
    ///
    /// * `tx` - 要签名的交易
    pub fn sign_transaction(&self, tx: &mut Transaction) {
        let secp = secp256k1::Secp256k1::new();
        let mut hasher = sha2::Sha256::new();
        let serialized = serde_json::to_string(tx).unwrap();
        hasher.update(serialized.as_bytes());
        let hash = hasher.finalize();
        
        let message = secp256k1::Message::from_slice(&hash).unwrap();
        let signature = secp.sign_ecdsa(&message, &self.private_key);
        
        for input in &mut tx.inputs {
            input.script_sig = format!("{}:{}", self.address, hex::encode(signature.serialize_compact()));
        }
    }

    /// 保存钱包到文件
    ///
    /// # 参数
    ///
    /// * `wallet` - 要保存的钱包实例
    /// * `filename` - 保存钱包的文件名
    pub fn save_wallet(wallet: &Wallet, filename: &str) {
        let serialized = serde_json::to_string(wallet).unwrap();
        fs::write(filename, serialized).expect("Unable to write wallet to file");
    }

    /// 从文件加载钱包
    ///
    /// # 参数
    ///
    /// * `filename` - 要加载的钱包文件名
    ///
    pub fn load_wallet(filename: &str) -> Wallet {
        let contents = fs::read_to_string(filename).expect("Unable to read wallet file");
        serde_json::from_str(&contents).expect("Unable to parse wallet file")
    } 
} 