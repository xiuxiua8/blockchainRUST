//! # Rust区块链演示项目
//! 
//! 这是一个使用Rust实现的区块链系统演示项目，提供了区块链的核心功能和组件。
//! 
//! ## 主要模块
//! 
//! * `block` - 定义区块、区块头和交易结构
//! * `blockchain` - 实现区块链和UTXO集合管理
//! * `wallet` - 提供密钥管理和交易签名功能
//! * `network` - 实现P2P网络通信功能

pub mod block;
pub mod blockchain;
pub mod wallet;
pub mod network;