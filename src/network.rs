use libp2p::{
    core::transport::Transport,
    identity,
    ping,
    swarm::{NetworkBehaviour, SwarmEvent, SwarmBuilder},
    PeerId,
    futures::StreamExt,
};
use tokio::sync::mpsc;
use std::collections::HashMap;
use crate::block::{Block, Transaction};
use crate::blockchain::Blockchain;

#[derive(Debug)]
pub enum NetworkEvent {
    NewBlock(Block),
    NewTransaction(Transaction),
    RequestBlocks,
    SendBlocks(Vec<Block>),
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "OutEvent")]
pub struct MyBehaviour {
    ping: ping::Behaviour,
}

#[derive(Debug)]
pub enum OutEvent {
    Ping(ping::Event),
}

impl From<ping::Event> for OutEvent {
    fn from(event: ping::Event) -> Self {
        OutEvent::Ping(event)
    }
}

pub struct Network {
    peer_id: PeerId,
    peers: HashMap<PeerId, String>,
    event_sender: mpsc::Sender<NetworkEvent>,
    event_receiver: mpsc::Receiver<NetworkEvent>,
}

impl Network {
    pub async fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        
        Network {
            peer_id,
            peers: HashMap::new(),
            event_sender,
            event_receiver,
        }
    }

    pub async fn start(&mut self) {
        let id_keys = identity::Keypair::generate_ed25519();
        
        let mut swarm = SwarmBuilder::with_tokio_executor(
            libp2p::development_transport(id_keys).await.unwrap(),
            MyBehaviour {
                ping: ping::Behaviour::new(ping::Config::new()),
            },
            self.peer_id,
        )
        .idle_connection_timeout(std::time::Duration::from_secs(60))
        .build();

        if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()) {
            eprintln!("Failed to start listening: {}", e);
            return;
        }

        while let Some(event) = swarm.next().await {
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address);
                }
                _ => {}
            }
        }
    }

    pub async fn broadcast_block(&self, block: Block) {
        if let Err(e) = self.event_sender.send(NetworkEvent::NewBlock(block)).await {
            eprintln!("Failed to broadcast block: {}", e);
        }
    }

    pub async fn broadcast_transaction(&self, transaction: Transaction) {
        if let Err(e) = self.event_sender.send(NetworkEvent::NewTransaction(transaction)).await {
            eprintln!("Failed to broadcast transaction: {}", e);
        }
    }

    pub async fn sync_chain(&self, _blockchain: &Blockchain) {
        if let Err(e) = self.event_sender.send(NetworkEvent::RequestBlocks).await {
            eprintln!("Failed to request blocks: {}", e);
        }
    }
} 