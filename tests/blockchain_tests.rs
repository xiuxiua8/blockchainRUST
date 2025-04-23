use blockchain_demo::block::{Transaction, TxInput, TxOutput};
use blockchain_demo::blockchain::Blockchain;
use std::fs;

#[test]
fn test_blockchain_add_block_and_utxo() {
    // 清理可能存在的测试文件
    let _ = fs::remove_file("test_blockchain.json");
    
    // 创建一个新的区块链实例
    let mut blockchain = Blockchain::new(2);
    
    // 记录初始区块数量
    let initial_block_count = blockchain.blocks.len();
    assert_eq!(initial_block_count, 1); // 应该有一个创世区块
    
    // 创建测试交易
    let tx_input = TxInput {
        prev_tx: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
        prev_index: 0,
        script_sig: String::from("测试签名"),
    };
    
    let tx_output = TxOutput {
        value: 50,
        script_pubkey: String::from("测试地址"),
    };
    
    let transaction = Transaction::new(vec![tx_input], vec![tx_output]);
    
    // 添加新区块
    blockchain.add_block(vec![transaction]);
    
    // 验证区块是否已添加
    assert_eq!(blockchain.blocks.len(), initial_block_count + 1);
    
    // 验证UTXO集合是否更新
    // 查找交易ID
    let tx_id = blockchain.calculate_tx_hash(&blockchain.blocks[1].transactions[0]);
    
    // 验证UTXO集合中是否存在该交易的输出
    assert!(blockchain.utxo_set.contains_key(&tx_id));
    
    // 验证输出的金额是否正确
    let outputs = blockchain.utxo_set.get(&tx_id).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].1, 50);
    
    // 添加第二个区块，消费第一个区块的UTXO
    let tx_input2 = TxInput {
        prev_tx: tx_id.clone(),
        prev_index: 0,
        script_sig: String::from("第二个交易的签名"),
    };
    
    let tx_output2 = TxOutput {
        value: 30,
        script_pubkey: String::from("接收者地址"),
    };
    
    let tx_output3 = TxOutput {
        value: 20,
        script_pubkey: String::from("找零地址"),
    };
    
    let transaction2 = Transaction::new(vec![tx_input2], vec![tx_output2, tx_output3]);
    
    // 添加包含第二个交易的区块
    blockchain.add_block(vec![transaction2]);
    
    // 验证UTXO集是否正确更新（第一个交易的输出应该被消费）
    assert!(!blockchain.utxo_set.get(&tx_id).unwrap().iter().any(|(idx, _)| *idx == 0));
    
    // 清理测试文件
    let _ = fs::remove_file("blockchain.json");
}
