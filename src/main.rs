//! # 区块链演示程序入口
//! 
//! 这是区块链演示项目的主程序入口，提供了一个简单的命令行界面，
//! 用于与区块链系统进行交互，包括创建交易、挖掘区块、查看余额和区块链状态等功能。

mod block;
mod blockchain;
mod wallet;
mod network;

use tokio::sync::mpsc;
use std::path::Path;
use std::io::{self, Write};
use tokio;
use std::collections::VecDeque;
use std::env;

use network::NetworkEvent;

/// 程序的主入口函数
///
/// 初始化区块链、钱包和网络组件，并启动命令行交互界面
#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let user_id = if args.len() > 1 { &args[1] } else { "user1" };
    
    // 使用user_id创建或加载钱包
    let wallet_file = format!("{}_wallet.json", user_id);
    let wallet = if Path::new(&wallet_file).exists() {
        // 从文件加载钱包
        wallet::Wallet::load_wallet(&wallet_file)
    } else {
        // 创建新钱包并保存
        let new_wallet = wallet::Wallet::new();
        wallet::Wallet::save_wallet(&new_wallet, &wallet_file);
        new_wallet
    };
    
    // 使用相同的链数据文件
    let blockchain_file = "blockchain.json";

    // 初始化日志
    env_logger::init();

    // 创建区块链
    //let mut blockchain = blockchain::Blockchain::new(2); // 难度为2
    let mut blockchain = blockchain::Blockchain::load_from_file(blockchain_file).unwrap();
    println!("Created new blockchain");

    // 创建网络和通道
    let (tx, mut rx) = mpsc::channel(100);
    let network = network::Network::new().await;
    let network_tx = tx.clone();

    let mut pending_transactions: VecDeque<block::Transaction> = VecDeque::new();

    // 启动网络在单独的任务中
    tokio::spawn(async move {
        let mut network = network;
        if let Err(e) = network.start().await {
            eprintln!("网络启动失败: {}", e);
        }
    });

    // 命令行界面
    loop {
        print!("\nBlockchain Demo Menu:\n");
        print!("1. Create new transaction\n");
        print!("2. Mine new block\n");
        print!("3. Show balance\n");
        print!("4. Show blockchain\n");
        print!("5. Show pending transactions\n");
        print!("6. Show all transactions\n");
        print!("7. Exit\n");
        print!("Enter your choice: ");
        io::stdout().flush().unwrap();
        
        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        
        match choice.trim() {
            "1" => {
                // 创建新交易
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
                    
                    // 添加到待处理交易池
                    pending_transactions.push_back(tx.clone());
                    
                    // 使用通道发送交易
                    if let Err(e) = network_tx.send(NetworkEvent::NewTransaction(tx)).await {
                        eprintln!("Failed to send transaction: {}", e);
                    }
                    println!("Transaction created and added to pending pool!");
                } else {
                    println!("Failed to create transaction: insufficient funds");
                }
            }
            "2" => {
                // 创建Coinbase交易（挖矿奖励）
                let coinbase_input = block::TxInput {
                    prev_tx: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
                    prev_index: 0,
                    script_sig: String::from("挖矿奖励"),
                };
                
                let coinbase_output = block::TxOutput {
                    value: 50, // 挖矿奖励
                    script_pubkey: wallet.address.clone(),
                };
                
                let coinbase_tx = block::Transaction::new(
                    vec![coinbase_input],
                    vec![coinbase_output]
                );
                
                // 从待处理交易池中获取交易
                let mut transactions = Vec::new();
                transactions.push(coinbase_tx);
                
                // 添加所有待处理的交易（或者最多 N 个）
                let max_tx_per_block = 10;
                let mut tx_count = 0;
                
                while !pending_transactions.is_empty() && tx_count < max_tx_per_block {
                    if let Some(tx) = pending_transactions.pop_front() {
                        transactions.push(tx);
                        tx_count += 1;
                    }
                }
                
                // 挖掘新区块
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
                // 显示余额
                println!("{}'s balance: {}", user_id ,blockchain.get_balance(user_id));
            }
            "4" => {
                // 显示区块链状态
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
                // 显示待处理交易
                println!("Pending Transactions: {}", pending_transactions.len());
                for (i, tx) in pending_transactions.iter().enumerate() {
                    println!("Transaction #{}", i);
                    // 显示交易详情
                }
            }
            "6" => {
                // 查询任意地址余额
                print!("Enter address to check: ");
                io::stdout().flush().unwrap();
                let mut check_address = String::new();
                io::stdin().read_line(&mut check_address).unwrap();
                
                let balance = blockchain.get_balance(check_address.trim());
                println!("Balance of {}: {}", check_address.trim(), balance);
            }
            "7" => {
                // 退出程序
                println!("Goodbye!");
                break;
            }
            _ => {
                println!("Invalid choice!");
            }
        }
    }
}

