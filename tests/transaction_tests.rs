use blockchain_demo::block::{Transaction, TxInput, TxOutput};
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hex;

// 辅助函数：计算交易哈希
fn calculate_tx_hash(tx: &Transaction) -> String {
    let mut hasher = Sha256::new();
    let serialized = serde_json::to_string(tx).unwrap();
    hasher.update(serialized.as_bytes());
    hex::encode(hasher.finalize())
}

#[test]
fn test_transaction_creation() {
    // 创建输入
    let tx_input = TxInput {
        prev_tx: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
        prev_index: 0,
        script_sig: String::from("测试签名"),
    };
    
    // 创建输出
    let tx_output = TxOutput {
        value: 50,
        script_pubkey: String::from("测试地址"),
    };
    
    // 创建交易
    let transaction = Transaction::new(vec![tx_input], vec![tx_output]);
    
    // 验证交易结构
    assert_eq!(transaction.inputs.len(), 1);
    assert_eq!(transaction.outputs.len(), 1);
    assert_eq!(transaction.inputs[0].prev_tx, "0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(transaction.inputs[0].prev_index, 0);
    assert_eq!(transaction.inputs[0].script_sig, "测试签名");
    assert_eq!(transaction.outputs[0].value, 50);
    assert_eq!(transaction.outputs[0].script_pubkey, "测试地址");
}

#[test]
fn test_transaction_with_multiple_inputs_outputs() {
    // 创建多个输入
    let tx_input1 = TxInput {
        prev_tx: String::from("1111111111111111111111111111111111111111111111111111111111111111"),
        prev_index: 0,
        script_sig: String::from("签名1"),
    };
    
    let tx_input2 = TxInput {
        prev_tx: String::from("2222222222222222222222222222222222222222222222222222222222222222"),
        prev_index: 1,
        script_sig: String::from("签名2"),
    };
    
    // 创建多个输出
    let tx_output1 = TxOutput {
        value: 30,
        script_pubkey: String::from("地址1"),
    };
    
    let tx_output2 = TxOutput {
        value: 20,
        script_pubkey: String::from("地址2"),
    };
    
    // 创建交易
    let transaction = Transaction::new(
        vec![tx_input1, tx_input2], 
        vec![tx_output1, tx_output2]
    );
    
    // 验证交易
    assert_eq!(transaction.inputs.len(), 2);
    assert_eq!(transaction.outputs.len(), 2);
    
    // 验证输入
    assert_eq!(transaction.inputs[0].prev_tx, "1111111111111111111111111111111111111111111111111111111111111111");
    assert_eq!(transaction.inputs[0].prev_index, 0);
    assert_eq!(transaction.inputs[0].script_sig, "签名1");
    
    assert_eq!(transaction.inputs[1].prev_tx, "2222222222222222222222222222222222222222222222222222222222222222");
    assert_eq!(transaction.inputs[1].prev_index, 1);
    assert_eq!(transaction.inputs[1].script_sig, "签名2");
    
    // 验证输出
    assert_eq!(transaction.outputs[0].value, 30);
    assert_eq!(transaction.outputs[0].script_pubkey, "地址1");
    
    assert_eq!(transaction.outputs[1].value, 20);
    assert_eq!(transaction.outputs[1].script_pubkey, "地址2");
    
    // 验证总输入和总输出
    let total_output = transaction.outputs.iter().fold(0, |acc, output| acc + output.value);
    assert_eq!(total_output, 50); // 30 + 20 = 50
}

#[test]
fn test_utxo_tracking() {
    // 创建第一个交易（类似于coinbase交易）
    let tx_input1 = TxInput {
        prev_tx: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
        prev_index: 0,
        script_sig: String::from("创世交易"),
    };
    
    let tx_output1 = TxOutput {
        value: 100,
        script_pubkey: String::from("地址A"),
    };
    
    let transaction1 = Transaction::new(vec![tx_input1], vec![tx_output1]);
    let tx1_id = calculate_tx_hash(&transaction1);
    
    // 模拟UTXO集
    let mut utxo_set: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    utxo_set.insert(tx1_id.clone(), vec![(0, 100)]);
    
    // 创建第二个交易，消费第一个交易的输出
    let tx_input2 = TxInput {
        prev_tx: tx1_id.clone(),
        prev_index: 0,
        script_sig: String::from("交易2的签名"),
    };
    
    let tx_output2 = TxOutput {
        value: 70,
        script_pubkey: String::from("地址B"),
    };
    
    let tx_output3 = TxOutput {
        value: 30,
        script_pubkey: String::from("地址A"), // 找零给自己
    };
    
    let transaction2 = Transaction::new(vec![tx_input2], vec![tx_output2, tx_output3]);
    let tx2_id = calculate_tx_hash(&transaction2);
    
    // 更新UTXO集
    // 1. 移除已花费的UTXO
    if let Some(outputs) = utxo_set.get_mut(&tx1_id) {
        outputs.retain(|&(idx, _)| idx != 0);
    }
    
    // 2. 如果交易的所有输出都被花费，移除整个条目
    utxo_set.retain(|_, outputs| !outputs.is_empty());
    
    // 3. 添加新交易的输出到UTXO集
    utxo_set.insert(tx2_id.clone(), vec![(0, 70), (1, 30)]);
    
    // 验证UTXO集
    assert!(!utxo_set.contains_key(&tx1_id)); // 第一个交易的输出已被花费
    assert!(utxo_set.contains_key(&tx2_id));  // 第二个交易的输出未被花费
    
    let outputs = utxo_set.get(&tx2_id).unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0], (0, 70));
    assert_eq!(outputs[1], (1, 30));
    
    // 验证UTXO集中的总值
    let total_value = utxo_set.values()
        .flat_map(|outputs| outputs.iter().map(|&(_, value)| value))
        .sum::<u64>();
    
    assert_eq!(total_value, 100); // 总值保持不变：70 + 30 = 100
} 