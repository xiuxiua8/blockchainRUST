# Rust区块链演示项目

一个使用Rust语言实现的完整区块链系统演示项目，包含区块链核心功能、交易系统、钱包和P2P网络。

## 项目特点

- **模块化设计**：清晰的模块划分，便于理解和扩展
- **核心功能实现**：
  - 区块和区块链结构
  - 工作量证明（PoW）挖矿
  - UTXO模型的交易系统
  - 数字签名和钱包功能
  - 基于libp2p的P2P网络
  - 区块链数据持久化

## 技术栈

- Rust 1.5+
- tokio异步运行时
- libp2p网络框架
- secp256k1椭圆曲线加密
- sha2哈希算法
- serde序列化/反序列化

## 快速开始

### 环境要求

- Rust 1.5+
- Cargo包管理器

### 安装和运行

```bash
# 克隆项目
git clone https://github.com/xiuxiua8/blockchainRUST.git
cd blockchainRUST

# 构建项目
cargo build

# 运行项目
cargo run
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行集成测试
cargo test --test integration_tests
```

## 项目结构

```
blockchain_demo/
├── src/
│   ├── block.rs       # 区块和交易结构
│   ├── blockchain.rs  # 区块链和UTXO集合
│   ├── wallet.rs      # 钱包和交易签名
│   ├── network.rs     # P2P网络功能
│   ├── main.rs        # 主程序入口
│   └── lib.rs         # 库入口和模块导出
├── tests/             # 测试目录
│   ├── block_tests.rs       # 区块测试
│   ├── blockchain_tests.rs  # 区块链测试
│   ├── wallet_tests.rs      # 钱包测试
│   ├── transaction_tests.rs # 交易测试
│   ├── network_tests.rs     # 网络测试
│   └── integration_tests.rs # 集成测试
├── tex/               # 文档目录
│   └── doc.tex        # LaTeX格式项目文档
└── Cargo.toml         # 项目配置和依赖
```

## 用法示例

项目支持以下主要功能：

1. 创建钱包
2. 生成区块链
3. 挖掘新区块
4. 创建和签名交易
5. 通过P2P网络广播区块和交易

详细用法请参考集成测试中的示例：[integration_tests.rs](tests/integration_tests.rs)

## 文档

详细的项目文档可以通过以下命令生成：

```bash
# 进入文档目录
cd tex

# 生成PDF文档（需要安装LaTeX）
pdflatex doc.tex
```

或者直接查看[doc.tex](tex/doc.tex)文件获取项目详细信息。

## 贡献指南

欢迎贡献代码！请遵循以下步骤：

1. Fork项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启Pull Request

## 许可证

MIT许可证 - 详见[LICENSE](LICENSE)文件

## 联系方式

项目维护者 - [1831894878@qq.com](mailto:1831894878@qq.com)

项目链接: [https://github.com/xiuxiua8/blockchainRUST](https://github.com/xiuxiua8/blockchainRUST) 