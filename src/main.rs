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
use std::fs;
use serde_json;
use std::sync::Arc;

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
    let blockchain = Arc::new(tokio::sync::Mutex::new(blockchain::Blockchain::new(2)));
    println!("Created new blockchain");

    // 创建网络和通道
    let (app_tx, mut app_rx) = mpsc::channel(100);
    let mut network = network::Network::new_with_channel(app_tx.clone()).await;
    
    // 创建一个共享的待处理交易池
    let pending_transactions: Arc<tokio::sync::Mutex<VecDeque<block::Transaction>>> = 
        Arc::new(tokio::sync::Mutex::new(VecDeque::new()));
    let pending_tx_for_network = pending_transactions.clone();
    let pending_tx_for_main = pending_transactions.clone();
    
    // 创建同步状态跟踪
    let sync_in_progress: Arc<tokio::sync::Mutex<bool>> = Arc::new(tokio::sync::Mutex::new(false));
    let sync_state_for_network = sync_in_progress.clone();
    
    // 获取节点ID
    let node_peer_id = network.peer_id();
    println!("节点ID: {}", node_peer_id);

    // 获取网络的事件发送器，用于发送应用层事件到网络
    let network_tx = network.get_event_sender();

    // 启动网络在单独的任务中
    tokio::spawn(async move {
        if let Err(e) = network.start().await {
            eprintln!("网络启动失败: {}", e);
        }
    });

    // 克隆必要的变量用于网络事件处理任务
    let blockchain_for_network = blockchain.clone();
    let network_tx_for_network = network_tx.clone();
    let pending_tx_for_network = pending_transactions.clone();
    let sync_state_for_task = sync_state_for_network.clone();

    // 网络事件处理任务
    tokio::spawn(async move {
        while let Some(event) = app_rx.recv().await {
            match event {
                NetworkEvent::NewBlock(block) => {
                    println!("\n📦 收到新区块: {}", block.calculate_hash());
                    
                    // 获取区块链的可变引用
                    let mut blockchain = blockchain_for_network.lock().await;
                    
                    // 验证区块
                    if blockchain.validate_block(&block) {
                        println!("✅ 区块验证通过，添加到本地区块链");
                        
                        // 添加区块到本地区块链
                        blockchain.add_received_block(block);
                        
                        println!("本地区块链已更新，当前高度: {}", blockchain.blocks.len());
                    } else {
                        println!("❌ 区块验证失败，可能需要同步区块链");
                        
                        // 区块验证失败时，自动请求区块链同步
                        drop(blockchain); // 释放锁
                        
                        println!("自动请求区块链同步...");
                        if let Err(e) = network_tx_for_network.send(NetworkEvent::RequestBlocks).await {
                            eprintln!("自动同步请求失败: {}", e);
                        } else {
                            println!("已发送区块链同步请求");
                        }
                    }
                },
                NetworkEvent::NewTransaction(transaction) => {
                    println!("\n💰 收到新交易");
                    println!("输入数量: {}", transaction.inputs.len());
                    println!("输出数量: {}", transaction.outputs.len());
                    
                    // 获取区块链的引用
                    let blockchain = blockchain_for_network.lock().await;
                    
                    // 验证交易
                    let is_valid = blockchain.validate_transaction(&transaction);
                    if is_valid {
                        println!("交易验证通过，添加到待处理池");
                        
                        // 获取待处理交易的可变引用
                        let mut pending_transactions = pending_tx_for_network.lock().await;
                        
                        // 检查交易是否已经在待处理池中
                        let tx_hash = transaction.calculate_hash();
                        let is_duplicate = pending_transactions.iter()
                            .any(|tx| tx.calculate_hash() == tx_hash);
                            
                        if !is_duplicate {
                            // 添加到待处理交易池
                            pending_transactions.push_back(transaction);
                            println!("交易已添加到待处理池");
                        } else {
                            println!("交易已存在于待处理池，忽略");
                        }
                    } else {
                        println!("交易验证失败，可能是UTXO状态不同步");
                        println!("暂时添加到待处理池，等待区块链同步后重新验证");
                        
                        // 释放区块链锁
                        drop(blockchain);
                        
                        // 获取待处理交易的可变引用
                        let mut pending_transactions = pending_tx_for_network.lock().await;
                        
                        // 检查交易是否已经在待处理池中
                        let tx_hash = transaction.calculate_hash();
                        let is_duplicate = pending_transactions.iter()
                            .any(|tx| tx.calculate_hash() == tx_hash);
                            
                        if !is_duplicate {
                            // 暂时添加到待处理交易池
                            pending_transactions.push_back(transaction);
                            println!("交易已暂时添加到待处理池");
                        }
                        
                        // 请求区块链同步
                        if let Err(e) = network_tx_for_network.send(NetworkEvent::RequestBlocks).await {
                            eprintln!("同步请求失败: {}", e);
                        } else {
                            println!("已发送区块链同步请求");
                        }
                    }
                },
                NetworkEvent::RequestBlocks => {
                    println!("\n📋 收到区块请求");
                    
                    // 获取区块链的引用
                    let blockchain = blockchain_for_network.lock().await;
                    
                    // 响应区块请求，发送本地区块链数据
                    let blocks_to_send = blockchain.blocks.clone();
                    println!("发送 {} 个区块作为响应", blocks_to_send.len());
                    
                    // 发送区块响应
                    if let Err(e) = network_tx_for_network.send(NetworkEvent::SendBlocks(blocks_to_send)).await {
                        eprintln!("发送区块响应失败: {}", e);
                    } else {
                        println!("区块响应已发送");
                    }
                },
                NetworkEvent::SendBlocks(blocks) => {
                    println!("\n📦 收到区块响应，总共 {} 个区块", blocks.len());
                    
                    if blocks.is_empty() {
                        println!("收到空区块列表，忽略");
                        return;
                    }
                    
                    // 获取区块链的可变引用
                    let mut blockchain = blockchain_for_network.lock().await;
                    
                    println!("本地区块链长度: {}, 收到的区块链长度: {}", blockchain.blocks.len(), blocks.len());
                    
                    // 智能同步检查：只有在收到的链更长时才进行同步
                    if blocks.len() > blockchain.blocks.len() {
                        println!("收到的区块链更长，开始验证和同步");
                        
                        // 创建临时区块链来验证整个链
                        let mut temp_blockchain = blockchain::Blockchain::new(blockchain.difficulty);
                        let mut is_valid_chain = true;
                        
                        // 验证整个区块链
                        for (i, block) in blocks.iter().enumerate() {
                            if i == 0 {
                                // 第一个区块（创世区块）
                                if block.header.prev_hash != "0" {
                                    println!("创世区块验证失败");
                                    is_valid_chain = false;
                                    break;
                                }
                                temp_blockchain.blocks.push(block.clone());
                                temp_blockchain.rebuild_utxo_set();
                            } else {
                                // 验证后续区块
                                if temp_blockchain.validate_block(block) {
                                    temp_blockchain.add_received_block(block.clone());
                                } else {
                                    is_valid_chain = false;
                                    println!("区块 #{} 验证失败", i);
                                    break;
                                }
                            }
                        }
                        
                        if is_valid_chain {
                            println!("收到的区块链有效，替换本地链");
                            
                            // 替换本地区块链
                            blockchain.replace_chain(blocks);
                            
                            // 更新UTXO集
                            blockchain.rebuild_utxo_set();
                            
                            println!("本地区块链已更新，当前高度: {}", blockchain.blocks.len());
                        } else {
                            println!("收到的区块链无效，保留本地链");
                        }
                    } else if blocks.len() == blockchain.blocks.len() {
                        // 检查是否是相同的链
                        let mut is_same_chain = true;
                        for (i, block) in blocks.iter().enumerate() {
                            if i < blockchain.blocks.len() {
                                let local_hash = blockchain.blocks[i].calculate_hash();
                                let received_hash = block.calculate_hash();
                                if local_hash != received_hash {
                                    is_same_chain = false;
                                    break;
                                }
                            }
                        }
                        
                        if is_same_chain {
                            println!("收到的区块链与本地链相同，无需同步");
                        } else {
                            println!("收到的区块链与本地链不同，但长度相同，保留本地链");
                        }
                    } else {
                        println!("收到的区块链比本地短，保留本地链");
                    }
                    
                    // 同步完成，重置同步状态
                    *sync_state_for_task.lock().await = false;
                },
                NetworkEvent::ConnectTo(_addr) => {
                    // 连接逻辑已经在network模块中处理
                },
                NetworkEvent::PeerDiscovered(peer_id, addr) => {
                    println!("\n🔍 发现新节点: {} at {}", peer_id, addr);
                },
                NetworkEvent::PeerConnected(peer_id) => {
                    println!("\n✅ 节点已连接: {}", peer_id);
                    
                    // 检查是否已经在同步中
                    let mut sync_in_progress = sync_state_for_task.lock().await;
                    if !*sync_in_progress {
                        *sync_in_progress = true;
                        drop(sync_in_progress); // 释放锁
                        
                        // 自动请求区块链同步
                        println!("自动请求区块链同步...");
                        if let Err(e) = network_tx_for_network.send(NetworkEvent::RequestBlocks).await {
                            eprintln!("自动同步请求失败: {}", e);
                            // 重置同步状态
                            *sync_state_for_task.lock().await = false;
                        } else {
                            println!("已发送区块链同步请求");
                        }
                    } else {
                        println!("同步已在进行中，跳过此次同步请求");
                    }
                },
                NetworkEvent::PeerDisconnected(peer_id) => {
                    println!("\n❌ 节点已断开: {}", peer_id);
                }
            }
        }
    });

    // 命令行界面
    loop {
        print!("\nBlockchain Demo Menu:\n");
        print!("1. Create new transaction\n");
        print!("2. Mine new block\n");
        print!("3. Show balance\n");
        print!("4. Show blockchain\n");
        print!("5. Exit\n");
        print!("6. Show pending transactions\n");
        print!("7. Show all transactions\n");
        print!("8. Connect to node\n");
        print!("9. Sync blockchain\n");
        print!("10. Show network status\n");
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
                
                // 获取区块链的锁以访问UTXO集
                let blockchain_lock = blockchain.lock().await;
                
                if let Some(mut tx) = wallet.create_transaction(
                    to_address.trim(),
                    amount,
                    &blockchain_lock.utxo_set,
                ) {
                    wallet.sign_transaction(&mut tx);
                    
                    // 释放区块链锁，不再需要
                    drop(blockchain_lock);
                    
                    // 添加到待处理交易池
                    pending_tx_for_main.lock().await.push_back(tx.clone());
                    
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
                
                while !pending_tx_for_main.lock().await.is_empty() && tx_count < max_tx_per_block {
                    if let Some(tx) = pending_tx_for_main.lock().await.pop_front() {
                        transactions.push(tx);
                        tx_count += 1;
                    }
                }
                
                // 挖掘新区块
                blockchain.lock().await.add_block(transactions);
                
                // 使用通道广播新区块
                if let Some(block) = blockchain.lock().await.blocks.last() {
                    if let Err(e) = network_tx.send(NetworkEvent::NewBlock(block.clone())).await {
                        eprintln!("Failed to broadcast block: {}", e);
                    }
                }
                println!("New block mined!");
            }
            "3" => {
                // 显示余额
                println!("{}'s balance: {}", user_id ,blockchain.lock().await.get_balance(&wallet.address));
            }
            "4" => {
                // 显示区块链状态
                println!("Blockchain:");
                for (i, block) in blockchain.lock().await.blocks.iter().enumerate() {
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
                // 退出程序
                println!("Goodbye!");
                break;
            }
            "6" => {
                // 显示待处理交易
                println!("Pending Transactions: {}", pending_tx_for_main.lock().await.len());
                for (i, tx) in pending_tx_for_main.lock().await.iter().enumerate() {
                    println!("Transaction #{}", i);
                    // 显示交易详情
                }
            }
            "7" => {
                // 查询任意地址余额
                print!("Enter address to check: ");
                io::stdout().flush().unwrap();
                let mut check_address = String::new();
                io::stdin().read_line(&mut check_address).unwrap();
                
                let balance = blockchain.lock().await.get_balance(check_address.trim());
                println!("Balance of {}: {}", check_address.trim(), balance);
            }
            "8" => {
                // 连接到其他节点
                print!("Enter node address to connect (e.g. /ip4/127.0.0.1/tcp/12345): ");
                io::stdout().flush().unwrap();
                let mut addr = String::new();
                io::stdin().read_line(&mut addr).unwrap();
                
                // 解析地址
                match addr.trim().parse::<libp2p::Multiaddr>() {
                    Ok(multiaddr) => {
                        // 发送连接请求
                        if let Err(e) = network_tx.send(NetworkEvent::ConnectTo(multiaddr.clone())).await {
                            eprintln!("发送连接请求失败: {}", e);
                        } else {
                            println!("已发送连接请求: {}", addr.trim());
                        }
                    },
                    Err(e) => {
                        eprintln!("地址格式错误: {}", e);
                    }
                }
            }
            "9" => {
                // 同步区块链
                println!("Requesting blockchain sync...");
                if let Err(e) = network_tx.send(NetworkEvent::RequestBlocks).await {
                    eprintln!("Failed to send block request: {}", e);
                } else {
                    println!("Block request sent!");
                }
            }
            "10" => {
                // 显示网络状态
                println!("\n=== 网络状态 ===");
                println!("节点ID: {}", node_peer_id);
                println!("自动连接功能已启用");
                println!("注意: 详细网络状态请查看控制台输出");
                println!("================\n");
            }
            _ => {
                println!("Invalid choice!");
            }
        }
    }
}