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
use std::collections::{VecDeque, HashMap};
use std::env;
use std::fs;
use serde_json;
use std::sync::Arc;

use network::NetworkEvent;

/// 地址解析函数，将用户友好的名称转换为钱包地址
async fn resolve_address(
    input: &str, 
    address_mapping: &Arc<tokio::sync::Mutex<HashMap<String, String>>>
) -> String {
    let mapping = address_mapping.lock().await;
    
    // 如果输入已经是有效的钱包地址（40个十六进制字符），直接返回
    if input.len() == 40 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        return input.to_string();
    }
    
    // 查找映射表
    if let Some(address) = mapping.get(input) {
        // 检查是否是占位符
        if address.ends_with("_placeholder") {
            println!("⚠️  警告: '{}' 是占位符地址，请使用菜单选项13更新为实际钱包地址", input);
            return address.clone();
        }
        return address.clone();
    }
    
    // 如果没有找到映射，返回原始输入（可能是新的地址）
    println!("ℹ️  未找到 '{}' 的地址映射，将作为原始地址使用", input);
    input.to_string()
}

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
    
    // 创建地址映射表，支持用户名和节点ID到钱包地址的映射
    let address_mapping: Arc<tokio::sync::Mutex<HashMap<String, String>>> = 
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let address_mapping_for_network = address_mapping.clone();
    let address_mapping_for_main = address_mapping.clone();
    
    // 添加当前用户的映射
    {
        let mut mapping = address_mapping.lock().await;
        mapping.insert(user_id.to_string(), wallet.address.clone());
        mapping.insert("me".to_string(), wallet.address.clone());
        mapping.insert("self".to_string(), wallet.address.clone());
        
        // 添加一些常用的用户名映射（用户可以通过菜单13更新）
        if user_id == "user1" {
            // 为user1添加user2的预设映射（需要用户手动更新为实际地址）
            mapping.insert("user2".to_string(), "user2_placeholder".to_string());
        } else if user_id == "user2" {
            // 为user2添加user1的预设映射（需要用户手动更新为实际地址）
            mapping.insert("user1".to_string(), "user1_placeholder".to_string());
        }
        
        println!("📝 地址映射已初始化:");
        println!("  {} -> {}", user_id, wallet.address);
        println!("  me -> {}", wallet.address);
        println!("  self -> {}", wallet.address);
    }
    
    // 创建同步状态跟踪
    let sync_in_progress: Arc<tokio::sync::Mutex<bool>> = Arc::new(tokio::sync::Mutex::new(false));
    let sync_state_for_network = sync_in_progress.clone();
    
    // 获取节点ID
    let node_peer_id = network.peer_id();
    println!("节点ID: {}", node_peer_id);

    // 获取网络的事件发送器，用于发送应用层事件到网络
    let network_tx = network.get_event_sender();
    
    // 创建网络实例的Arc包装，用于在主循环中访问网络信息
    let network_for_main = Arc::new(tokio::sync::Mutex::new(network));
    let network_for_start = network_for_main.clone();

    // 启动网络在单独的任务中
    tokio::spawn(async move {
        let mut network = network_for_start.lock().await;
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
                        blockchain.add_received_block(block.clone());
                        
                        println!("本地区块链已更新，当前高度: {}", blockchain.blocks.len());
                        
                        // 释放区块链锁，避免死锁
                        drop(blockchain);
                        
                        // 从待处理交易池中移除已经被打包的交易
                        let mut pending_transactions = pending_tx_for_network.lock().await;
                        let initial_count = pending_transactions.len();
                        
                        // 获取区块中的所有交易哈希
                        let block_tx_hashes: std::collections::HashSet<String> = block.transactions.iter()
                            .map(|tx| tx.calculate_hash())
                            .collect();
                        
                        // 保留不在区块中的交易
                        pending_transactions.retain(|tx| {
                            let tx_hash = tx.calculate_hash();
                            !block_tx_hashes.contains(&tx_hash)
                        });
                        
                        let removed_count = initial_count - pending_transactions.len();
                        if removed_count > 0 {
                            println!("🗑️ 从待处理池中移除了 {} 个已确认的交易", removed_count);
                            println!("📊 待处理交易池剩余: {} 个交易", pending_transactions.len());
                        }
                        
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
                    println!("\n📋 收到区块同步请求（来自网络）");
                    
                    // 获取区块链的引用
                    let blockchain = blockchain_for_network.lock().await;
                    
                    // 发送本地区块链数据作为响应
                    let blocks_to_send = blockchain.blocks.clone();
                    println!("响应网络同步请求，发送 {} 个区块", blocks_to_send.len());
                    
                    // 释放区块链锁
                    drop(blockchain);
                    
                    // 通过网络发送区块链数据响应
                    if let Err(e) = network_tx_for_network.send(NetworkEvent::SendBlocks(blocks_to_send)).await {
                        eprintln!("发送区块链响应失败: {}", e);
                    } else {
                        println!("区块链响应已发送");
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
                            blockchain.replace_chain(blocks.clone());
                            
                            // 更新UTXO集
                            blockchain.rebuild_utxo_set();
                            
                            println!("本地区块链已更新，当前高度: {}", blockchain.blocks.len());
                            
                            // 释放区块链锁
                            drop(blockchain);
                            
                            // 更新待处理交易池，移除已经被确认的交易
                            let mut pending_transactions = pending_tx_for_network.lock().await;
                            let initial_count = pending_transactions.len();
                            
                            // 收集所有区块中的交易哈希
                            let mut confirmed_tx_hashes = std::collections::HashSet::new();
                            for block in &blocks {
                                for tx in &block.transactions {
                                    confirmed_tx_hashes.insert(tx.calculate_hash());
                                }
                            }
                            
                            // 保留不在任何区块中的交易
                            pending_transactions.retain(|tx| {
                                let tx_hash = tx.calculate_hash();
                                !confirmed_tx_hashes.contains(&tx_hash)
                            });
                            
                            let removed_count = initial_count - pending_transactions.len();
                            if removed_count > 0 {
                                println!("🗑️ 同步后从待处理池中移除了 {} 个已确认的交易", removed_count);
                                println!("📊 待处理交易池剩余: {} 个交易", pending_transactions.len());
                            }
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
                    
                    // 自动添加节点ID到地址映射表（暂时映射到节点ID本身，用户可以后续更新）
                    {
                        let mut mapping = address_mapping_for_network.lock().await;
                        let peer_id_str = peer_id.to_string();
                        if !mapping.contains_key(&peer_id_str) {
                            // 暂时将节点ID映射到自己，用户可以通过菜单选项13更新为实际钱包地址
                            mapping.insert(peer_id_str.clone(), peer_id_str.clone());
                            println!("📝 节点ID已添加到地址映射表: {}", peer_id);
                            println!("💡 提示: 你可以使用菜单选项13将此节点ID映射到实际钱包地址");
                        }
                    }
                    
                    // 检查是否已经在同步中
                    let mut sync_in_progress = sync_state_for_task.lock().await;
                    if !*sync_in_progress {
                        *sync_in_progress = true;
                        drop(sync_in_progress); // 释放锁
                        
                        // 等待一下让Gossipsub建立网格连接
                        println!("等待网格连接建立...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        
                        // 发送网络同步请求（通过网络广播）
                        println!("发送网络同步请求...");
                        if let Err(e) = network_tx_for_network.send(NetworkEvent::RequestBlocks).await {
                            eprintln!("发送网络同步请求失败: {}", e);
                            // 重置同步状态
                            *sync_state_for_task.lock().await = false;
                        } else {
                            println!("网络同步请求已发送");
                        }
                    } else {
                        println!("同步已在进行中，跳过此次同步请求");
                    }
                },
                NetworkEvent::PeerDisconnected(peer_id) => {
                    println!("\n❌ 节点已断开: {}", peer_id);
                },
                NetworkEvent::ConnectionInfo { connected_peers, all_peers } => {
                    // 处理连接信息响应
                    println!("当前节点ID: {}", node_peer_id);
                    println!("连接状态: {} 个连接", connected_peers.len());
                    println!();
                    
                    if connected_peers.is_empty() {
                        println!("❌ 当前没有连接到任何节点");
                    } else {
                        println!("✅ 已连接的节点:");
                        for (peer_id, addr) in &connected_peers {
                            println!("  📱 节点ID: {}", peer_id);
                            if let Some(address) = addr {
                                println!("     网络地址: {}", address);
                            }
                            
                            // 查找地址映射
                            let mapping = address_mapping_for_network.lock().await;
                            let peer_id_str = peer_id.to_string();
                            if let Some(mapped_addr) = mapping.get(&peer_id_str) {
                                if mapped_addr != &peer_id_str {
                                    println!("     钱包地址: {}", mapped_addr);
                                } else {
                                    println!("     钱包地址: 未设置 (使用菜单13添加映射)");
                                }
                            }
                            
                            // 查找用户名映射
                            let mut user_names = Vec::new();
                            for (name, addr) in mapping.iter() {
                                if addr == &peer_id_str && name != &peer_id_str {
                                    user_names.push(name.clone());
                                }
                            }
                            if !user_names.is_empty() {
                                println!("     用户名: {}", user_names.join(", "));
                            }
                            println!();
                        }
                    }
                    
                    // 显示已发现但未连接的节点
                    let disconnected_peers: Vec<_> = all_peers.iter()
                        .filter(|(_, _, is_connected)| !is_connected)
                        .collect();
                        
                    if !disconnected_peers.is_empty() {
                        println!("🔍 已发现但未连接的节点:");
                        for (peer_id, addr, _) in disconnected_peers {
                            println!("  📱 节点ID: {}", peer_id);
                            println!("     网络地址: {}", addr);
                            
                            // 查找地址映射
                            let mapping = address_mapping_for_network.lock().await;
                            let peer_id_str = peer_id.to_string();
                            if let Some(mapped_addr) = mapping.get(&peer_id_str) {
                                if mapped_addr != &peer_id_str {
                                    println!("     钱包地址: {}", mapped_addr);
                                }
                            }
                            println!();
                        }
                    }
                    
                    // 显示地址映射统计
                    let mapping = address_mapping_for_network.lock().await;
                    println!("📋 地址映射统计:");
                    println!("  总映射数: {}", mapping.len());
                    let placeholder_count = mapping.values().filter(|v| v.ends_with("_placeholder")).count();
                    if placeholder_count > 0 {
                        println!("  占位符映射: {} (需要更新)", placeholder_count);
                    }
                    
                    println!("================\n");
                },
                _ => {}
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
        print!("11. Debug UTXO set\n");
        print!("12. Show address mapping\n");
        print!("13. Add address mapping\n");
        print!("14. Show connected users\n");
        print!("Enter your choice: ");
        io::stdout().flush().unwrap();
        
        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        
        match choice.trim() {
            "1" => {
                // 创建新交易
                print!("Enter recipient address (支持: 钱包地址/用户名/节点ID): ");
                io::stdout().flush().unwrap();
                let mut to_address = String::new();
                io::stdin().read_line(&mut to_address).unwrap();
                
                // 解析地址
                let resolved_address = resolve_address(to_address.trim(), &address_mapping_for_main).await;
                
                print!("Enter amount: ");
                io::stdout().flush().unwrap();
                let mut amount = String::new();
                io::stdin().read_line(&mut amount).unwrap();
                
                let amount: u64 = amount.trim().parse().unwrap();
                
                // 获取区块链的锁以访问UTXO集
                let blockchain_lock = blockchain.lock().await;
                
                if let Some(mut tx) = wallet.create_transaction(
                    &resolved_address,
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
                    println!("发送给: {} (解析为: {})", to_address.trim(), resolved_address);
                } else {
                    println!("Failed to create transaction: insufficient funds");
                    println!("目标地址: {} (解析为: {})", to_address.trim(), resolved_address);
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
            "11" => {
                // 调试UTXO集
                print!("Enter address to debug (or press Enter for current user): ");
                io::stdout().flush().unwrap();
                let mut debug_address = String::new();
                io::stdin().read_line(&mut debug_address).unwrap();
                
                let address_to_debug = if debug_address.trim().is_empty() {
                    wallet.address.clone()
                } else {
                    debug_address.trim().to_string()
                };
                
                let blockchain_lock = blockchain.lock().await;
                blockchain_lock.debug_utxo_set(&address_to_debug);
            }
            "12" => {
                // 显示地址映射表
                println!("\n=== 地址映射表 ===");
                let mapping = address_mapping.lock().await;
                for (key, value) in mapping.iter() {
                    println!("{}: {}", key, value);
                }
                println!("================\n");
            }
            "13" => {
                // 添加地址映射
                print!("Enter address to map: ");
                io::stdout().flush().unwrap();
                let mut new_address = String::new();
                io::stdin().read_line(&mut new_address).unwrap();
                
                print!("Enter mapped address: ");
                io::stdout().flush().unwrap();
                let mut mapped_address = String::new();
                io::stdin().read_line(&mut mapped_address).unwrap();
                
                let mut mapping = address_mapping.lock().await;
                mapping.insert(new_address.trim().to_string(), mapped_address.trim().to_string());
                println!("地址映射已添加");
            }
            "14" => {
                // 显示连接用户信息
                println!("\n=== 连接用户信息 ===");
                
                // 发送连接信息请求
                if let Err(e) = network_tx.send(NetworkEvent::RequestConnectionInfo).await {
                    eprintln!("发送连接信息请求失败: {}", e);
                } else {
                    println!("正在获取连接信息...");
                }
            }
            _ => {
                println!("Invalid choice!");
            }
        }
    }
}