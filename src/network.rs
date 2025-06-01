//! # 网络模块
//! 
//! 实现区块链的点对点(P2P)网络功能，包括节点发现、区块和交易广播等功能。
//! 
//! 该模块基于libp2p库构建，提供了分布式网络通信的基础设施。

use libp2p::{
    identity,
    ping,
    swarm::{NetworkBehaviour, SwarmEvent, Swarm},
    PeerId,
    futures::StreamExt,
    gossipsub,
    mdns,
    kad,
    Multiaddr,
};
use tokio::sync::mpsc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::error::Error;
use serde::{Serialize, Deserialize};
use crate::block::{Block, Transaction};
use crate::blockchain::Blockchain;

/// 网络事件枚举，表示节点间可以传递的消息类型
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// 新区块事件，包含一个完整的区块
    NewBlock(Block),
    /// 新交易事件，包含一个待处理的交易
    NewTransaction(Transaction),
    /// 请求区块事件，向其他节点请求区块数据
    RequestBlocks,
    /// 发送区块事件，响应区块请求
    SendBlocks(Vec<Block>),
    /// 连接到指定地址的节点
    ConnectTo(libp2p::Multiaddr),
    /// 发现新节点事件
    PeerDiscovered(PeerId, Multiaddr),
    /// 节点连接事件
    PeerConnected(PeerId),
    /// 节点断开事件
    PeerDisconnected(PeerId),
}

/// 网络消息包装结构，用于网络传输
#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// 区块消息
    Block(Block),
    /// 交易消息
    Transaction(Transaction),
    /// 区块请求消息
    BlockRequest,
    /// 区块响应消息
    BlockResponse(Vec<Block>),
}

/// 自定义网络行为事件类型
#[derive(Debug)]
pub enum MyBehaviourEvent {
    /// Ping事件
    Ping(ping::Event),
    /// Gossipsub事件
    Gossipsub(gossipsub::Event),
    /// mDNS事件
    Mdns(mdns::Event),
    /// Kademlia事件
    Kademlia(kad::Event),
}

impl From<ping::Event> for MyBehaviourEvent {
    fn from(event: ping::Event) -> Self {
        MyBehaviourEvent::Ping(event)
    }
}

impl From<gossipsub::Event> for MyBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        MyBehaviourEvent::Gossipsub(event)
    }
}

impl From<mdns::Event> for MyBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        MyBehaviourEvent::Mdns(event)
    }
}

impl From<kad::Event> for MyBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        MyBehaviourEvent::Kademlia(event)
    }
}

/// 网络行为定义，实现了libp2p的NetworkBehaviour trait
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "MyBehaviourEvent")]
pub struct MyBehaviour {
    /// ping行为，用于检测节点连接状态
    ping: ping::Behaviour,
    /// gossipsub 行为，用于区块链消息广播
    gossipsub: gossipsub::Behaviour,
    /// mDNS 行为，用于本地网络节点发现
    mdns: mdns::tokio::Behaviour,
    /// Kademlia DHT 行为，用于分布式节点发现
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

/// 网络结构，封装P2P网络功能
pub struct Network {
    /// 节点ID
    peer_id: PeerId,
    /// 已知节点列表，键为节点ID，值为节点地址
    peers: HashMap<PeerId, String>,
    /// 连接的节点集合
    connected_peers: HashSet<PeerId>,
    /// 事件发送器，用于向网络发送事件
    event_sender: mpsc::Sender<NetworkEvent>,
    /// 事件接收器，用于接收网络事件
    event_receiver: mpsc::Receiver<NetworkEvent>,
    /// 区块主题
    blocks_topic: gossipsub::IdentTopic,
    /// 交易主题
    transactions_topic: gossipsub::IdentTopic,
    /// libp2p swarm实例
    swarm: Option<Swarm<MyBehaviour>>,
    /// 自动连接开关
    auto_connect_enabled: bool,
    /// 最大连接数
    max_connections: usize,
    /// 应用层事件发送器
    app_event_sender: Option<mpsc::Sender<NetworkEvent>>,
}

impl Network {
    /// 创建新的网络实例
    ///
    /// # 返回值
    ///
    /// 返回初始化的网络实例
    pub async fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        
        let blocks_topic = gossipsub::IdentTopic::new("blocks");
        let transactions_topic = gossipsub::IdentTopic::new("transactions");
        
        Network {
            peer_id,
            peers: HashMap::new(),
            connected_peers: HashSet::new(),
            event_sender,
            event_receiver,
            blocks_topic,
            transactions_topic,
            swarm: None,
            auto_connect_enabled: true,
            max_connections: 10,
            app_event_sender: None,
        }
    }

    /// 启用或禁用自动连接
    pub fn set_auto_connect(&mut self, enabled: bool) {
        self.auto_connect_enabled = enabled;
        println!("自动连接已{}", if enabled { "启用" } else { "禁用" });
    }

    /// 设置最大连接数
    pub fn set_max_connections(&mut self, max: usize) {
        self.max_connections = max;
        println!("最大连接数设置为: {}", max);
    }

    /// 启动网络服务
    ///
    /// 初始化libp2p swarm并开始监听网络事件
    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        // 使用简化方法创建 swarm
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let peer_id = PeerId::from(key.public());
                self.peer_id = peer_id;
                
                // 配置 gossipsub
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .expect("有效的 gossipsub 配置");
                    
                // 创建 gossipsub 行为
                let mut gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                ).expect("创建 gossipsub 行为失败");
                    
                // 订阅主题
                gossipsub.subscribe(&self.blocks_topic)
                    .expect("订阅区块主题失败");
                gossipsub.subscribe(&self.transactions_topic)
                    .expect("订阅交易主题失败");

                // 创建 mDNS 行为
                let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)
                    .expect("创建 mDNS 行为失败");

                // 创建 Kademlia DHT 行为
                let store = kad::store::MemoryStore::new(peer_id);
                let kademlia = kad::Behaviour::new(peer_id, store);
                
                Ok(MyBehaviour {
                    ping: ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)).with_timeout(Duration::from_secs(20))),
                    gossipsub,
                    mdns,
                    kademlia,
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
            .build();

        // 开始监听
        // 尝试一系列固定端口
        println!("尝试绑定到固定端口...");
        let fixed_ports = vec![40000, 40001, 40002, 40003, 40004, 40005, 40006, 40007, 40008, 40009, 40010];
        let mut listen_success = false;
        
        for port in fixed_ports {
            println!("尝试端口 {}...", port);
            let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", port);
            
            match swarm.listen_on(listen_addr.parse()?) {
                Ok(_) => {
                    println!("成功监听在端口 {}", port);
                    listen_success = true;
                    break;
                },
                Err(e) => {
                    println!("端口 {} 绑定失败: {}", port, e);
                    // 继续尝试下一个端口
                }
            }
        }
        
        // 如果所有固定端口都失败，尝试随机端口
        if !listen_success {
            println!("所有固定端口都绑定失败，尝试使用随机端口...");
            if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?) {
                eprintln!("启动监听失败: {}", e);
                return Err(e.into());
            }
        }

        println!("P2P 网络启动，节点 ID: {}", self.peer_id);
        
        // 等待监听地址分配
        println!("等待监听地址分配...");
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("分配的监听地址: {}", address);
                    break;
                }
                _ => {}
            }
        }

        // 显示节点信息
        println!("==========================================================");
        println!("P2P 节点已启动");
        println!("节点ID: {}", self.peer_id);
        if let Some(addr) = swarm.listeners().next() {
            println!("监听地址: {}", addr);
            println!("其他节点可以通过菜单选项8连接到此地址");
            if self.auto_connect_enabled {
                println!("自动连接已启用，将自动发现并连接到其他节点");
            }
        }
        println!("==========================================================");

        // 存储swarm实例
        self.swarm = Some(swarm);

        // 主事件循环
        self.run_event_loop().await
    }

    /// 运行主事件循环
    async fn run_event_loop(&mut self) -> Result<(), Box<dyn Error>> {
        let mut swarm = self.swarm.take().unwrap();
        
        loop {
            tokio::select! {
                // 处理应用层事件
                event = self.event_receiver.recv() => {
                    if let Some(event) = event {
                        self.handle_application_event(&mut swarm, event).await?;
                    }
                }
                
                // 处理网络事件
                event = swarm.select_next_some() => {
                    self.handle_swarm_event(&mut swarm, event).await?;
                }
            }
        }
    }

    /// 处理应用层事件
    async fn handle_application_event(
        &mut self,
        swarm: &mut Swarm<MyBehaviour>,
        event: NetworkEvent,
    ) -> Result<(), Box<dyn Error>> {
        match event {
            NetworkEvent::NewBlock(block) => {
                println!("广播新区块: {}", block.calculate_hash());
                let message = NetworkMessage::Block(block);
                let data = serde_json::to_vec(&message)?;
                
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
                    eprintln!("广播区块失败: {}", e);
                }
            }
            NetworkEvent::NewTransaction(transaction) => {
                println!("广播新交易");
                let message = NetworkMessage::Transaction(transaction);
                let data = serde_json::to_vec(&message)?;
                
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.transactions_topic.clone(), data) {
                    eprintln!("广播交易失败: {}", e);
                }
            }
            NetworkEvent::RequestBlocks => {
                println!("广播区块请求");
                let message = NetworkMessage::BlockRequest;
                let data = serde_json::to_vec(&message)?;
                
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
                    eprintln!("广播区块请求失败: {}", e);
                }
            }
            NetworkEvent::SendBlocks(blocks) => {
                println!("广播区块响应，包含 {} 个区块", blocks.len());
                let message = NetworkMessage::BlockResponse(blocks);
                let data = serde_json::to_vec(&message)?;
                
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
                    eprintln!("广播区块响应失败: {}", e);
                }
            }
            NetworkEvent::ConnectTo(addr) => {
                println!("尝试连接到: {}", addr);
                if let Err(e) = swarm.dial(addr.clone()) {
                    eprintln!("连接失败: {}", e);
                } else {
                    println!("连接请求已发送");
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// 处理Swarm网络事件
    async fn handle_swarm_event(
        &mut self,
        swarm: &mut Swarm<MyBehaviour>,
        event: SwarmEvent<MyBehaviourEvent, libp2p::swarm::THandlerErr<MyBehaviour>>,
    ) -> Result<(), Box<dyn Error>> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("正在监听地址: {}", address);
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    println!("🔍 mDNS发现新节点: {} at {}", peer_id, multiaddr);
                    
                    // 自动连接到发现的节点
                    if self.auto_connect_enabled && 
                       !self.connected_peers.contains(&peer_id) && 
                       self.connected_peers.len() < self.max_connections {
                        
                        println!("🔗 自动连接到发现的节点: {}", peer_id);
                        if let Err(e) = swarm.dial(multiaddr.clone()) {
                            eprintln!("自动连接失败: {}", e);
                        }
                    }
                    
                    // 添加到Kademlia路由表
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr.clone());
                    
                    // 存储节点信息
                    self.peers.insert(peer_id, multiaddr.to_string());
                }
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _multiaddr) in list {
                    println!("📤 mDNS节点过期: {}", peer_id);
                    self.peers.remove(&peer_id);
                    self.connected_peers.remove(&peer_id);
                }
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
                match result {
                    kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { peers, .. })) => {
                        println!("🌐 Kademlia发现 {} 个节点", peers.len());
                        for peer in peers {
                            if self.auto_connect_enabled && 
                               !self.connected_peers.contains(&peer) && 
                               self.connected_peers.len() < self.max_connections {
                                
                                // 尝试通过已知地址连接
                                if let Some(addr_str) = self.peers.get(&peer) {
                                    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                                        println!("🔗 通过Kademlia自动连接到: {} at {}", peer, addr);
                                        if let Err(e) = swarm.dial(addr) {
                                            eprintln!("Kademlia自动连接失败: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("✅ 已连接到节点: {}", peer_id);
                self.connected_peers.insert(peer_id);
                
                // 发送连接事件
                if let Err(e) = self.event_sender.send(NetworkEvent::PeerConnected(peer_id)).await {
                    eprintln!("发送连接事件失败: {}", e);
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                println!("❌ 与节点断开连接: {}", peer_id);
                self.connected_peers.remove(&peer_id);
                
                // 发送断开事件
                if let Err(e) = self.event_sender.send(NetworkEvent::PeerDisconnected(peer_id)).await {
                    eprintln!("发送断开事件失败: {}", e);
                }
                
                // 自动重连机制
                if self.auto_connect_enabled && self.connected_peers.len() < self.max_connections {
                    if let Some(addr_str) = self.peers.get(&peer_id) {
                        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                            println!("🔄 尝试自动重连到: {}", peer_id);
                            
                            // 延迟重连，避免立即重连
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            
                            if let Err(e) = swarm.dial(addr) {
                                eprintln!("自动重连失败: {}", e);
                            } else {
                                println!("已发送自动重连请求");
                            }
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: _peer_id,
                message_id: _id,
                message,
            })) => {
                // 处理接收到的gossipsub消息
                match serde_json::from_slice::<NetworkMessage>(&message.data) {
                    Ok(NetworkMessage::Block(block)) => {
                        println!("📦 收到区块广播: {}", block.calculate_hash());
                        // 转发到应用层
                        if let Some(app_sender) = &self.app_event_sender {
                            if let Err(e) = app_sender.send(NetworkEvent::NewBlock(block)).await {
                                eprintln!("转发区块事件到应用层失败: {}", e);
                            }
                        }
                    }
                    Ok(NetworkMessage::Transaction(transaction)) => {
                        println!("💰 收到交易广播");
                        // 转发到应用层
                        if let Some(app_sender) = &self.app_event_sender {
                            if let Err(e) = app_sender.send(NetworkEvent::NewTransaction(transaction)).await {
                                eprintln!("转发交易事件到应用层失败: {}", e);
                            }
                        }
                    }
                    Ok(NetworkMessage::BlockRequest) => {
                        println!("📋 收到区块请求");
                        // 转发到应用层
                        if let Some(app_sender) = &self.app_event_sender {
                            if let Err(e) = app_sender.send(NetworkEvent::RequestBlocks).await {
                                eprintln!("转发区块请求到应用层失败: {}", e);
                            }
                        }
                    }
                    Ok(NetworkMessage::BlockResponse(blocks)) => {
                        println!("📦 收到区块响应，包含 {} 个区块", blocks.len());
                        // 转发到应用层
                        if let Some(app_sender) = &self.app_event_sender {
                            if let Err(e) = app_sender.send(NetworkEvent::SendBlocks(blocks)).await {
                                eprintln!("转发区块响应到应用层失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("解析网络消息失败: {}", e);
                    }
                }
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Ping(ping_event)) => {
                // 简化ping事件处理，只记录连接活跃状态
                println!("🏓 Ping事件: {:?}", ping_event);
            }
            _ => {}
        }
        Ok(())
    }

    /// 获取节点ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// 获取事件发送器
    pub fn get_event_sender(&self) -> mpsc::Sender<NetworkEvent> {
        self.event_sender.clone()
    }

    /// 获取连接的节点数量
    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }

    /// 获取已发现的节点数量
    pub fn discovered_peer_count(&self) -> usize {
        self.peers.len()
    }

    /// 手动触发节点发现
    pub async fn discover_peers(&mut self) {
        if let Some(swarm) = &mut self.swarm {
            // 启动Kademlia查询来发现更多节点
            let _ = swarm.behaviour_mut().kademlia.get_closest_peers(self.peer_id);
            println!("🔍 启动节点发现查询...");
        }
    }

    /// 显示网络状态
    pub fn show_network_status(&self) {
        println!("\n=== 网络状态 ===");
        println!("节点ID: {}", self.peer_id);
        println!("已连接节点: {}", self.connected_peers.len());
        println!("已发现节点: {}", self.peers.len());
        println!("自动连接: {}", if self.auto_connect_enabled { "启用" } else { "禁用" });
        println!("最大连接数: {}", self.max_connections);
        
        if !self.connected_peers.is_empty() {
            println!("连接的节点:");
            for peer in &self.connected_peers {
                println!("  - {}", peer);
            }
        }
        
        if !self.peers.is_empty() {
            println!("发现的节点:");
            for (peer, addr) in &self.peers {
                let status = if self.connected_peers.contains(peer) { "已连接" } else { "未连接" };
                println!("  - {} ({}) - {}", peer, status, addr);
            }
        }
        println!("================\n");
    }

    // 保留原有的方法以保持兼容性
    pub async fn broadcast_block(&self, block: Block) {
        if let Err(e) = self.event_sender.send(NetworkEvent::NewBlock(block)).await {
            eprintln!("发送区块广播事件失败: {}", e);
        }
    }

    pub async fn broadcast_transaction(&self, transaction: Transaction) {
        if let Err(e) = self.event_sender.send(NetworkEvent::NewTransaction(transaction)).await {
            eprintln!("发送交易广播事件失败: {}", e);
        }
    }

    pub async fn sync_chain(&self, _blockchain: &Blockchain) {
        if let Err(e) = self.event_sender.send(NetworkEvent::RequestBlocks).await {
            eprintln!("发送区块同步请求失败: {}", e);
        }
    }

    pub async fn new_with_channel(app_event_sender: mpsc::Sender<NetworkEvent>) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        
        let blocks_topic = gossipsub::IdentTopic::new("blocks");
        let transactions_topic = gossipsub::IdentTopic::new("transactions");
        
        Network {
            peer_id,
            peers: HashMap::new(),
            connected_peers: HashSet::new(),
            event_sender,
            event_receiver,
            blocks_topic,
            transactions_topic,
            swarm: None,
            auto_connect_enabled: true,
            max_connections: 10,
            app_event_sender: Some(app_event_sender),
        }
    }

    pub async fn dial(&self, addr: libp2p::Multiaddr) -> Result<(), Box<dyn Error>> {
        if let Err(e) = self.event_sender.send(NetworkEvent::ConnectTo(addr)).await {
            eprintln!("发送连接请求失败: {}", e);
            return Err(e.into());
        }
        Ok(())
    }
} 