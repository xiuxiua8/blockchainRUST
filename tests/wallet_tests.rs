use blockchain_demo::wallet::Wallet;
use blockchain_demo::block::{Transaction, TxInput, TxOutput};
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hex;

// 辅助函数：计算交易哈希，用于测试
fn calculate_tx_hash(tx: &Transaction) -> String {
    let mut hasher = Sha256::new();
    let serialized = serde_json::to_string(tx).unwrap();
    hasher.update(serialized.as_bytes());
    hex::encode(hasher.finalize())
}

#[test]
fn test_wallet_creation() {
    // 创建新钱包
    let wallet = Wallet::new();
    
    // 验证钱包地址不为空
    assert!(!wallet.address.is_empty());
    
    // 验证钱包地址是有效的十六进制字符串（40个字符，20字节的RIPEMD160哈希）
    assert_eq!(wallet.address.len(), 40);
    assert!(wallet.address.chars().all(|c| c.is_ascii_hexdigit()));
    
    // 创建另一个钱包，验证地址唯一性
    let wallet2 = Wallet::new();
    assert_ne!(wallet.address, wallet2.address);
}

#[test]
fn test_transaction_creation_with_sufficient_funds() {
    // 创建钱包
    let wallet = Wallet::new();
    
    // 模拟UTXO集合
    let mut utxo_set: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    utxo_set.insert("tx1".to_string(), vec![(0, 100)]);
    
    // 创建交易，金额小于可用资金
    let to_address = "recipient_address";
    let amount = 50;
    
    let tx_option = wallet.create_transaction(to_address, amount, &utxo_set);
    assert!(tx_option.is_some());
    
    let tx = tx_option.unwrap();
    
    // 验证交易输入
    assert_eq!(tx.inputs.len(), 1);
    assert_eq!(tx.inputs[0].prev_tx, "tx1");
    assert_eq!(tx.inputs[0].prev_index, 0);
    
    // 验证交易输出
    assert_eq!(tx.outputs.len(), 2); // 一个给接收者，一个找零
    assert_eq!(tx.outputs[0].value, amount);
    assert_eq!(tx.outputs[0].script_pubkey, to_address);
    assert_eq!(tx.outputs[1].value, 100 - amount); // 找零
    assert_eq!(tx.outputs[1].script_pubkey, wallet.address);
}

#[test]
fn test_transaction_creation_with_exact_funds() {
    // 创建钱包
    let wallet = Wallet::new();
    
    // 模拟UTXO集合
    let mut utxo_set: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    utxo_set.insert("tx1".to_string(), vec![(0, 50)]);
    
    // 创建交易，金额刚好等于可用资金
    let to_address = "recipient_address";
    let amount = 50;
    
    let tx_option = wallet.create_transaction(to_address, amount, &utxo_set);
    assert!(tx_option.is_some());
    
    let tx = tx_option.unwrap();
    
    // 验证交易输入
    assert_eq!(tx.inputs.len(), 1);
    
    // 验证交易输出 - 只有一个输出，没有找零
    assert_eq!(tx.outputs.len(), 1);
    assert_eq!(tx.outputs[0].value, amount);
    assert_eq!(tx.outputs[0].script_pubkey, to_address);
}

#[test]
fn test_transaction_creation_with_insufficient_funds() {
    // 创建钱包
    let wallet = Wallet::new();
    
    // 模拟UTXO集合
    let mut utxo_set: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    utxo_set.insert("tx1".to_string(), vec![(0, 30)]);
    
    // 创建交易，金额大于可用资金
    let to_address = "recipient_address";
    let amount = 50;
    
    let tx_option = wallet.create_transaction(to_address, amount, &utxo_set);
    
    // 资金不足应该返回None
    assert!(tx_option.is_none());
}

#[test]
fn test_transaction_creation_with_multiple_inputs() {
    // 创建钱包
    let wallet = Wallet::new();
    
    // 模拟UTXO集合，多个UTXO
    let mut utxo_set: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    utxo_set.insert("tx1".to_string(), vec![(0, 30)]);
    utxo_set.insert("tx2".to_string(), vec![(0, 20), (1, 10)]);
    
    // 创建交易，需要多个输入才能满足金额
    let to_address = "recipient_address";
    let amount = 50;
    
    let tx_option = wallet.create_transaction(to_address, amount, &utxo_set);
    assert!(tx_option.is_some());
    
    let tx = tx_option.unwrap();
    
    // 验证交易输入 - 应该收集足够的输入
    assert!(tx.inputs.len() >= 2); // 至少需要两个输入
    
    // 验证总输入金额
    let mut total_input = 0;
    for input in &tx.inputs {
        if input.prev_tx == "tx1" && input.prev_index == 0 {
            total_input += 30;
        } else if input.prev_tx == "tx2" && input.prev_index == 0 {
            total_input += 20;
        } else if input.prev_tx == "tx2" && input.prev_index == 1 {
            total_input += 10;
        }
    }
    assert!(total_input >= amount);
    
    // 验证交易输出
    let total_output = tx.outputs.iter().fold(0, |acc, output| acc + output.value);
    assert_eq!(total_output, total_input); // 输入和输出应该平衡
}

#[test]
fn test_transaction_signing() {
    // 创建钱包
    let wallet = Wallet::new();
    
    // 创建简单的交易
    let tx_input = TxInput {
        prev_tx: "tx1".to_string(),
        prev_index: 0,
        script_sig: wallet.address.clone(), // 初始签名只是地址
    };
    
    let tx_output = TxOutput {
        value: 50,
        script_pubkey: "recipient_address".to_string(),
    };
    
    let mut tx = Transaction::new(vec![tx_input], vec![tx_output]);
    
    // 交易签名前的script_sig
    let original_script_sig = tx.inputs[0].script_sig.clone();
    
    // 签名交易
    wallet.sign_transaction(&mut tx);
    
    // 签名后script_sig应该已更改
    assert_ne!(tx.inputs[0].script_sig, original_script_sig);
    
    // 签名后的script_sig应该包含钱包地址
    assert!(tx.inputs[0].script_sig.starts_with(&wallet.address));
    
    // 签名后的script_sig应该包含":"，格式为"地址:签名"
    assert!(tx.inputs[0].script_sig.contains(':'));
    
    // 签名部分应该是有效的十六进制字符串
    let parts: Vec<&str> = tx.inputs[0].script_sig.split(':').collect();
    assert_eq!(parts.len(), 2);
    let signature_hex = parts[1];
    assert!(signature_hex.chars().all(|c| c.is_ascii_hexdigit()));
} 