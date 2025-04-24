use blockchain_demo::block::{Block, Transaction, TxInput, TxOutput};

#[test]
fn test_block_mining_and_validation() {
    // 创建一个新区块，使用较低的难度值
    let mut block = Block::new(String::from("0000000000000000000000000000000000000000000000000000000000000000"), 3);
    
    // 添加一个测试交易
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
    block.transactions.push(transaction);
    
    // 初始状态下区块应该无效
    assert_eq!(block.is_valid(), false);
    
    // 挖矿
    block.mine();
    
    // 挖矿后区块应该有效
    assert_eq!(block.is_valid(), true);
    
    // 验证挖矿是否改变了nonce值
    assert!(block.header.nonce > 0);
    
    // 验证哈希值是否满足难度要求（前缀有足够的0）
    let hash = block.calculate_hash();
    let required_prefix = "0".repeat(block.header.difficulty as usize);
    assert!(hash.starts_with(&required_prefix));
}
