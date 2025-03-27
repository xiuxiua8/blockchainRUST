#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::blockchain::Blockchain;
    use crate::transaction::{Transaction, TxInput, TxOutput, UTXO};
    use crate::wallet::Wallet;
    use std::path::PathBuf;

    #[test]
    fn test_block_creation() {
        let prev_hash = "0".repeat(64);
        let block = Block::new(vec![], prev_hash.clone());
        
        assert_eq!(block.prev_block_hash, prev_hash);
        assert_eq!(block.transactions.len(), 0);
        assert!(!block.hash.is_empty());
    }

    #[test]
    fn test_block_mining() {
        let prev_hash = "0".repeat(64);
        let mut block = Block::new(vec![], prev_hash);
        let difficulty = 4;
        
        block.mine(difficulty);
        
        assert!(block.hash.starts_with(&"0".repeat(difficulty)));
    }

    #[test]
    fn test_blockchain_creation() {
        let difficulty = 4;
        let blockchain = Blockchain::new(difficulty);
        
        assert_eq!(blockchain.chain.len(), 1); // 创世区块
        assert_eq!(blockchain.difficulty, difficulty);
    }

    #[test]
    fn test_blockchain_validity() {
        let difficulty = 4;
        let mut blockchain = Blockchain::new(difficulty);
        
        // 添加一个新区块
        blockchain.add_block(vec![]);
        
        assert!(blockchain.is_valid());
    }

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new();
        
        assert!(!wallet.private_key.is_empty());
        assert!(!wallet.public_key.is_empty());
        assert!(!wallet.address.is_empty());
    }

    #[test]
    fn test_transaction_creation() {
        let inputs = vec![TxInput {
            tx_id: "test_tx".to_string(),
            output_index: 0,
            signature: "test_signature".to_string(),
        }];
        
        let outputs = vec![TxOutput {
            value: 100,
            pub_key_hash: "test_pub_key_hash".to_string(),
        }];
        
        let transaction = Transaction::new(inputs, outputs);
        
        assert!(!transaction.id.is_empty());
        assert_eq!(transaction.inputs.len(), 1);
        assert_eq!(transaction.outputs.len(), 1);
    }

    #[test]
    fn test_utxo_set() {
        let mut utxo_set = UTXOSet::new();
        
        let utxo = UTXO {
            tx_id: "test_tx".to_string(),
            output_index: 0,
            value: 100,
            pub_key_hash: "test_pub_key_hash".to_string(),
        };
        
        utxo_set.add_utxo(utxo.clone());
        assert_eq!(utxo_set.utxos.len(), 1);
        
        utxo_set.remove_utxo(&utxo.tx_id, utxo.output_index);
        assert_eq!(utxo_set.utxos.len(), 0);
    }

    #[test]
    fn test_blockchain_persistence() {
        let difficulty = 4;
        let mut blockchain = Blockchain::new(difficulty);
        blockchain.add_block(vec![]);
        
        let test_path = PathBuf::from("test_blockchain.json");
        blockchain.save_to_file(&test_path).unwrap();
        
        let loaded_blockchain = Blockchain::load_from_file(&test_path).unwrap();
        assert_eq!(loaded_blockchain.chain.len(), blockchain.chain.len());
        assert_eq!(loaded_blockchain.difficulty, blockchain.difficulty);
        
        // 清理测试文件
        std::fs::remove_file(&test_path).unwrap();
    }
} 