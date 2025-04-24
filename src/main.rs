mod block;
mod blockchain;
mod wallet;
mod network;

use tokio::sync::mpsc;
use std::path::Path;
use std::io::{self, Write};
use tokio;

use network::NetworkEvent;

#[tokio::main]
async fn main() {
    // 初始化日志
    env_logger::init();

    // 创建区块链
    let mut blockchain = blockchain::Blockchain::new(2); // 难度为4
    println!("Created new blockchain");

    // 创建钱包
    let wallet = wallet::Wallet::new();
    println!("Created wallet with address: {}", wallet.address);

    // 创建网络和通道
    let (tx, mut rx) = mpsc::channel(100);
    let network = network::Network::new().await;
    let network_tx = tx.clone();

    // 启动网络在单独的任务中
    tokio::spawn(async move {
        let mut network = network;
        network.start().await;
    });

    // 命令行界面
    loop {
        print!("\nBlockchain Demo Menu:\n");
        print!("1. Create new transaction\n");
        print!("2. Mine new block\n");
        print!("3. Show balance\n");
        print!("4. Show blockchain\n");
        print!("5. Exit\n");
        print!("Enter your choice: ");
        io::stdout().flush().unwrap();
        
        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        
        match choice.trim() {
            "1" => {
                print!("Enter recipient address: ");
                io::stdout().flush().unwrap();
                let mut to_address = String::new();
                io::stdin().read_line(&mut to_address).unwrap();
                
                print!("Enter amount: ");
                io::stdout().flush().unwrap();
                let mut amount = String::new();
                io::stdin().read_line(&mut amount).unwrap();
                
                let amount: u64 = amount.trim().parse().unwrap();
                
                if let Some(mut tx) = wallet.create_transaction(
                    to_address.trim(),
                    amount,
                    &blockchain.utxo_set,
                ) {
                    wallet.sign_transaction(&mut tx);
                    // 使用通道发送交易
                    if let Err(e) = network_tx.send(NetworkEvent::NewTransaction(tx.clone())).await {
                        eprintln!("Failed to send transaction: {}", e);
                    }
                    println!("Transaction created and broadcasted!");
                } else {
                    println!("Failed to create transaction: insufficient funds");
                }
            }
            "2" => {
                let mut transactions = Vec::new();
                blockchain.add_block(transactions);
                // 使用通道广播新区块
                if let Some(block) = blockchain.blocks.last() {
                    if let Err(e) = network_tx.send(NetworkEvent::NewBlock(block.clone())).await {
                        eprintln!("Failed to broadcast block: {}", e);
                    }
                }
                println!("New block mined!");
            }
            "3" => {
                println!("Your balance: {}", blockchain.get_balance(&wallet.address));
            }
            "4" => {
                println!("Blockchain:");
                for (i, block) in blockchain.blocks.iter().enumerate() {
                    println!("Block #{}", i);
                    println!("  Hash: {}", block.calculate_hash());
                    println!("  Previous Hash: {}", block.header.prev_hash);
                    println!("  Timestamp: {}", block.header.timestamp);
                    println!("  Nonce: {}", block.header.nonce);
                    println!("  Transactions: {}", block.transactions.len());
                    println!();
                }
            }
            "5" => {
                println!("Goodbye!");
                break;
            }
            _ => {
                println!("Invalid choice!");
            }
        }
    }
} 