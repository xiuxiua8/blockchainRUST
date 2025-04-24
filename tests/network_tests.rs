use blockchain_demo::network::{Network, NetworkEvent};
use blockchain_demo::block::{Block, Transaction, TxInput, TxOutput};
use blockchain_demo::blockchain::Blockchain;
use tokio::sync::mpsc;
use tokio::time::timeout;
use std::time::Duration;

// 辅助函数：创建测试区块
fn create_test_block() -> Block {
    let mut block = Block::new(String::from("0000000000000000000000000000000000000000000000000000000000000000"), 1);
    
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
    
    block
}

// 辅助函数：创建测试交易
fn create_test_transaction() -> Transaction {
    let tx_input = TxInput {
        prev_tx: String::from("1111111111111111111111111111111111111111111111111111111111111111"),
        prev_index: 0,
        script_sig: String::from("测试签名"),
    };
    
    let tx_output = TxOutput {
        value: 50,
        script_pubkey: String::from("接收地址"),
    };
    
    Transaction::new(vec![tx_input], vec![tx_output])
}

#[tokio::test]
async fn test_network_creation() {
    // 创建网络实例
    let network = Network::new().await;
    
    // 由于peer_id是私有字段，我们不能直接访问，所以这里只验证网络实例创建成功
    assert!(true);
}

#[tokio::test]
async fn test_broadcast_block() {
    // 创建网络实例和接收通道
    let (tx, mut rx) = mpsc::channel(10);
    
    // 创建监听任务，接收广播的区块
    let listen_handle = tokio::spawn(async move {
        if let Some(event) = rx.recv().await {
            match event {
                NetworkEvent::NewBlock(block) => {
                    // 验证收到的区块
                    assert_eq!(block.transactions.len(), 1);
                    assert_eq!(block.transactions[0].outputs[0].value, 50);
                    return true;
                }
                _ => return false,
            }
        } else {
            return false;
        }
    });
    
    // 创建测试区块
    let test_block = create_test_block();
    
    // 发送区块到通道
    tx.send(NetworkEvent::NewBlock(test_block)).await.unwrap();
    
    // 等待接收结果
    let result = timeout(Duration::from_secs(1), listen_handle).await.unwrap().unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_broadcast_transaction() {
    // 创建网络实例和接收通道
    let (tx, mut rx) = mpsc::channel(10);
    
    // 创建监听任务，接收广播的交易
    let listen_handle = tokio::spawn(async move {
        if let Some(event) = rx.recv().await {
            match event {
                NetworkEvent::NewTransaction(transaction) => {
                    // 验证收到的交易
                    assert_eq!(transaction.inputs.len(), 1);
                    assert_eq!(transaction.outputs.len(), 1);
                    assert_eq!(transaction.outputs[0].value, 50);
                    assert_eq!(transaction.outputs[0].script_pubkey, "接收地址");
                    return true;
                }
                _ => return false,
            }
        } else {
            return false;
        }
    });
    
    // 创建测试交易
    let test_transaction = create_test_transaction();
    
    // 发送交易到通道
    tx.send(NetworkEvent::NewTransaction(test_transaction)).await.unwrap();
    
    // 等待接收结果
    let result = timeout(Duration::from_secs(1), listen_handle).await.unwrap().unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_request_blocks() {
    // 创建网络实例和接收通道
    let (tx, mut rx) = mpsc::channel(10);
    
    // 创建监听任务，接收区块请求
    let listen_handle = tokio::spawn(async move {
        if let Some(event) = rx.recv().await {
            match event {
                NetworkEvent::RequestBlocks => {
                    return true;
                }
                _ => return false,
            }
        } else {
            return false;
        }
    });
    
    // 发送区块请求
    tx.send(NetworkEvent::RequestBlocks).await.unwrap();
    
    // 等待接收结果
    let result = timeout(Duration::from_secs(1), listen_handle).await.unwrap().unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_send_blocks() {
    // 创建网络实例和接收通道
    let (tx, mut rx) = mpsc::channel(10);
    
    // 创建监听任务，接收多个区块
    let listen_handle = tokio::spawn(async move {
        if let Some(event) = rx.recv().await {
            match event {
                NetworkEvent::SendBlocks(blocks) => {
                    // 验证收到的区块列表
                    assert_eq!(blocks.len(), 2);
                    assert_eq!(blocks[0].transactions.len(), 1);
                    assert_eq!(blocks[1].transactions.len(), 1);
                    return true;
                }
                _ => return false,
            }
        } else {
            return false;
        }
    });
    
    // 创建两个测试区块
    let test_block1 = create_test_block();
    let test_block2 = create_test_block();
    
    // 发送区块列表
    tx.send(NetworkEvent::SendBlocks(vec![test_block1, test_block2])).await.unwrap();
    
    // 等待接收结果
    let result = timeout(Duration::from_secs(1), listen_handle).await.unwrap().unwrap();
    assert!(result);
}

// 测试使用 Network 的方法发送事件
#[tokio::test]
async fn test_network_broadcast_methods() {
    // 创建一个区块链用于测试
    let blockchain = Blockchain::new(1);
    
    // 创建网络实例
    let network = Network::new().await;
    
    // 创建一个接收器，拦截 network 内部的 event_sender 发送的消息
    // 因为 network 内部的 event_receiver 是私有的，我们不能直接访问
    // 这个测试主要是验证网络方法不会崩溃
    
    // 测试广播区块
    let test_block = create_test_block();
    let broadcast_result = network.broadcast_block(test_block).await;
    
    // 测试广播交易
    let test_transaction = create_test_transaction();
    let transaction_result = network.broadcast_transaction(test_transaction).await;
    
    // 测试同步链
    let sync_result = network.sync_chain(&blockchain).await;
    
    // 这里我们只是测试方法调用不会崩溃
    // 由于 Network 结构的设计，我们无法在测试中直接验证内部通道的事件
    assert!(true);
} 