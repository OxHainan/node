# mp Executor Engine

[![Crates.io](https://img.shields.io/crates/v/mp-executor-engine.svg)](https://crates.io/crates/mp-executor-engine)
[![Documentation](https://docs.rs/mp-executor-engine/badge.svg)](https://docs.rs/mp-executor-engine)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

mp-executor-engine 是一个高性能、可靠的 TCP 通信引擎，专为容器间通信设计，支持自动重连和错误恢复机制。

## 功能特点

- **可靠的 TCP 通信**：支持在不同容器之间建立稳定的 TCP 连接
- **自动重连机制**：在连接断开时自动尝试重新建立连接
- **错误处理**：全面处理各种网络错误，包括 BrokenPipe、ConnectionReset 等
- **异步设计**：基于 Tokio 的异步架构，高效处理并发连接
- **请求-响应模式**：支持完整的请求-响应通信模式
- **连接状态监控**：提供连接状态检查和监控功能

## 架构设计

mp-executor-engine 采用读写分离的设计模式，主要包含以下核心组件：

- **ComputerEngineReader**：负责异步读取来自远程容器的消息
- **ComputerEngineWriter**：负责向远程容器发送消息
- **ComputerEngineWorker**：管理多个容器连接，处理消息路由和重连逻辑
- **EngineService**：提供高级 API 接口，处理请求和响应

## 快速开始

### 安装

将以下依赖添加到您的 `Cargo.toml` 文件中：

```toml
[dependencies]
mp-executor-engine = "0.1.0"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
serde_json = "1.0"
```

### 基本用法

以下是一个基本的使用示例：

```rust
use mp_executor_engine::{ComputerEngineWorker, Method, Params, Status};
use std::net::SocketAddr;
use tokio::runtime::Handle;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 获取当前 Tokio 运行时句柄
    let handle = Handle::current();
    
    // 创建 ComputerEngineWorker 实例
    let mut engine = ComputerEngineWorker::new(handle.clone())?
        .with_auto_reconnect(true)
        .with_max_reconnect_attempts(5)
        .with_reconnect_interval(1000);
    
    // 连接到远程容器
    let remote_id = Uuid::new_v4();
    let container_addr = "127.0.0.1:3010".parse::<SocketAddr>()?;
    engine.connect(remote_id, container_addr).await?;
    
    // 获取服务引用，给其他模块进行调用
    let service = engine.service().clone();
    
    // 启动 worker
    handle.spawn(engine.run());
    
    // 为每一次请求分配一个 UUID
    let request_id = Uuid::new_v4();
    let response = service
        .request(
            request_id,
            &remote_id,
            serde_json::json!("Hello from client"),
            None,
        )
        .await?;
    
    println!("收到响应: {:?}", response);
    
    Ok(())
}
```

## 高级功能

### 异步消息请求

```rust
// 发送异步消息请求
let request_id = Uuid::new_v4();
let response = service
    .request(
        request_id,
        &remote_id,
        serde_json::json!("Hello from client"),
        None,
    )
    .await?;
```

### 自动重连配置

```rust
// 启用自动重连
let worker = ComputerEngineWorker::new(handle)?
    .with_auto_reconnect(true)
    .with_max_reconnect_attempts(10)  // 最大重试次数
    .with_reconnect_interval(2000);   // 重连间隔（毫秒）
```

### 连接状态检查

```rust
// 检查连接状态
if let Some(engine) = worker.get_engine(&remote_id) {
    if engine.is_running() {
        println!("连接正常");
    } else {
        println!("连接已断开");
    }
}
```

### 获取已连接容器列表

```rust
// 获取所有已连接的容器
let containers = worker.get_connected_containers();
for container in containers {
    println!("已连接容器: {}", container);
}
```

## 错误处理

mp-executor-engine 提供了全面的错误处理机制，主要处理以下错误类型：

- **BrokenPipe**：连接被对方关闭
- **ConnectionReset**：连接被重置
- **NotConnected**：尝试向未连接的目标发送数据
- **其他 IO 错误**：网络超时、拒绝连接等

当检测到连接断开时，如果启用了自动重连功能，系统会自动尝试重新建立连接。

## 性能优化

- **异步 IO**：使用 Tokio 的异步 IO 操作，避免阻塞
- **锁优化**：使用 try_lock 先尝试获取锁，失败时再使用异步等待
- **缓冲区管理**：合理管理读写缓冲区，减少内存分配

## 示例

查看 [examples](https://github.com/your-org/mp-executor-engine/tree/main/examples) 目录获取更多使用示例：

- **test.rs**：演示如何创建服务器和客户端，以及如何处理连接断开和重连

## 贡献

欢迎提交 Pull Request 和 Issue！

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件
