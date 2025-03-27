use secp256k1::{PublicKey, SecretKey};
use sha2::{Sha256, Digest};
use hex;
use std::collections::HashMap;
use crate::block::{Transaction, TxInput, TxOutput};
use rand;

pub struct Wallet {
    pub private_key: SecretKey,
    pub public_key: PublicKey,
    pub address: String,
}

impl Wallet {
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
} 