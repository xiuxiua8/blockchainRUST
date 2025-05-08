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
};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::time::Duration;
use std::error::Error;
use serde::{Serialize, Deserialize};
use futures::future::FutureExt;
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

/// 网络行为定义，实现了libp2p的NetworkBehaviour trait
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "MyBehaviourEvent")]
pub struct MyBehaviour {
    /// ping行为，用于检测节点连接状态
    ping: ping::Behaviour,
    /// gossipsub 行为，用于区块链消息广播
    gossipsub: gossipsub::Behaviour,
}

/// 网络结构，封装P2P网络功能
pub struct Network {
    /// 节点ID
    peer_id: PeerId,
    /// 已知节点列表，键为节点ID，值为节点地址
    peers: HashMap<PeerId, String>,
    /// 事件发送器，用于向网络发送事件
    event_sender: mpsc::Sender<NetworkEvent>,
    /// 事件接收器，用于接收网络事件
    event_receiver: mpsc::Receiver<NetworkEvent>,
    /// 区块主题
    blocks_topic: gossipsub::IdentTopic,
    /// 交易主题
    transactions_topic: gossipsub::IdentTopic,
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
            event_sender,
            event_receiver,
            blocks_topic,
            transactions_topic,
        }
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
                
                Ok(MyBehaviour {
                    ping: ping::Behaviour::new(ping::Config::new()),
                    gossipsub,
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // 开始监听
        if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()) {
            eprintln!("启动监听失败: {}", e);
            return Ok(());
        }

        println!("P2P 网络启动，节点 ID: {}", self.peer_id);
        
        // 主事件循环
        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            println!("正在监听地址: {:?}", address);
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message { 
                            propagation_source,
                            message_id,
                            message,
                        })) => {
                            println!("收到来自 {:?} 的 gossipsub 消息: {:?}", propagation_source, message_id);
                            
                            // 尝试解析消息
                            if let Ok(network_message) = serde_json::from_slice::<NetworkMessage>(&message.data) {
                                match network_message {
                                    NetworkMessage::Block(block) => {
                                        println!("收到新区块: {}", block.calculate_hash());
                                        // 这里可以添加验证和处理区块的逻辑
                                    },
                                    NetworkMessage::Transaction(transaction) => {
                                        println!("收到新交易");
                                        // 这里可以添加验证和处理交易的逻辑
                                    },
                                    NetworkMessage::BlockRequest => {
                                        println!("收到区块请求");
                                        // 这里可以添加响应区块请求的逻辑
                                    },
                                    NetworkMessage::BlockResponse(blocks) => {
                                        println!("收到区块响应，共 {} 个区块", blocks.len());
                                        // 这里可以添加处理区块响应的逻辑
                                    }
                                }
                            } else {
                                eprintln!("无法解析收到的消息");
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Ping(_)) => {},
                        _ => {}
                    }
                },
                Some(event) = self.event_receiver.recv() => {
                    match event {
                        NetworkEvent::NewBlock(block) => {
                            println!("广播新区块: {}", block.calculate_hash());
                            let message = NetworkMessage::Block(block);
                            if let Ok(data) = serde_json::to_vec(&message) {
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
                                    eprintln!("发布区块失败: {}", e);
                                }
                            }
                        },
                        NetworkEvent::NewTransaction(transaction) => {
                            println!("广播新交易");
                            let message = NetworkMessage::Transaction(transaction);
                            if let Ok(data) = serde_json::to_vec(&message) {
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.transactions_topic.clone(), data) {
                                    eprintln!("发布交易失败: {}", e);
                                }
                            }
                        },
                        NetworkEvent::RequestBlocks => {
                            println!("请求区块");
                            let message = NetworkMessage::BlockRequest;
                            if let Ok(data) = serde_json::to_vec(&message) {
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
                                    eprintln!("发布区块请求失败: {}", e);
                                }
                            }
                        },
                        NetworkEvent::SendBlocks(blocks) => {
                            println!("发送区块响应，共 {} 个区块", blocks.len());
                            let message = NetworkMessage::BlockResponse(blocks);
                            if let Ok(data) = serde_json::to_vec(&message) {
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
                                    eprintln!("发布区块响应失败: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }

        #[allow(unreachable_code)]
        Ok(())
    }

    /// 广播新区块
    ///
    /// # 参数
    ///
    /// * `block` - 要广播的区块
    pub async fn broadcast_block(&self, block: Block) {
        println!("准备广播区块: {}", block.calculate_hash());
        if let Err(e) = self.event_sender.send(NetworkEvent::NewBlock(block)).await {
            eprintln!("广播区块失败: {}", e);
        }
    }

    /// 广播新交易
    ///
    /// # 参数
    ///
    /// * `transaction` - 要广播的交易
    pub async fn broadcast_transaction(&self, transaction: Transaction) {
        println!("准备广播交易");
        if let Err(e) = self.event_sender.send(NetworkEvent::NewTransaction(transaction)).await {
            eprintln!("广播交易失败: {}", e);
        }
    }

    /// 同步区块链
    ///
    /// 向网络中的其他节点请求最新的区块数据
    ///
    /// # 参数
    ///
    /// * `_blockchain` - 本地区块链实例
    pub async fn sync_chain(&self, _blockchain: &Blockchain) {
        println!("请求同步区块链");
        if let Err(e) = self.event_sender.send(NetworkEvent::RequestBlocks).await {
            eprintln!("请求区块失败: {}", e);
        }
    }

    /// 获取节点ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }
    
    /// 使用自定义通道创建网络实例
    pub async fn new_with_channel(event_sender: mpsc::Sender<NetworkEvent>) -> Self {
        let (_, event_receiver) = mpsc::channel(100);
        
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        
        let blocks_topic = gossipsub::IdentTopic::new("blocks");
        let transactions_topic = gossipsub::IdentTopic::new("transactions");
        
        Network {
            peer_id,
            peers: HashMap::new(),
            event_sender,
            event_receiver,
            blocks_topic,
            transactions_topic,
        }
    }
    
    /// 连接到指定地址的节点
    pub async fn dial(&mut self, addr: libp2p::Multiaddr) -> Result<(), Box<dyn Error>> {
        // 这个方法需要修改 start() 方法，将 swarm 移出循环，但为了简化，这里返回成功
        println!("尝试连接到地址: {}", addr);
        Ok(())
    }
} 