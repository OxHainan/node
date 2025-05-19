use mp_executor_engine::service::EngineRequest;
use mp_executor_engine::{ComputerEngineWorker, Method, Params, Status};
use std::net::SocketAddr;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

async fn start_docker(server_id: usize, addr: SocketAddr) {
    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind socket");
    println!("🚀 [Docker {}] Listening on {:?}", server_id, addr);

    // 每个服务器的连接计数器
    let mut connection_counter = 0;

    loop {
        match listener.accept().await {
            Ok((stream, client_addr)) => {
                connection_counter += 1;
                println!(
                    "🔌 [Docker {}] 接受来自 {} 的连接 (第 {} 个连接)",
                    server_id, client_addr, connection_counter
                );

                let server_id = server_id.clone();
                // 处理客户端连接
                tokio::spawn(async move {
                    // 模拟服务器处理连接
                    let mut message_counter = 0;
                    let (mut reader, mut writer) = stream.into_split();

                    loop {
                        // 读取消息长度
                        let mut length_buf = [0u8; 4];
                        match reader.read_exact(&mut length_buf).await {
                            Ok(_) => {
                                // 解析消息长度
                                let length = u32::from_le_bytes(length_buf) as usize;

                                // 读取消息内容
                                let mut buffer = vec![0u8; length];
                                match reader.read_exact(&mut buffer).await {
                                    Ok(_) => {
                                        // 尝试解析为Params结构体
                                        match serde_json::from_slice::<Params>(&buffer) {
                                            Ok(params) => {
                                                message_counter += 1;
                                                println!(
                                                    "📥 [Docker {}] 收到消息 #{}: {:?}",
                                                    server_id, message_counter, params
                                                );

                                                // 创建响应
                                                let response = Params {
                                                    id: params.id, // 使用相同ID
                                                    method: Method::Invoke,
                                                    status: Status::RUNNING, // 设置为运行状态
                                                    data: serde_json::json!({
                                                        "response": format!("✅ [Docker {}] 已收到消息 #{}", server_id, message_counter),
                                                        "server_id": server_id
                                                    }),
                                                };

                                                // 序列化响应
                                                let response_data =
                                                    serde_json::to_vec(&response).unwrap();
                                                let response_length =
                                                    (response_data.len() as u32).to_le_bytes();

                                                // 发送响应长度和数据
                                                let mut combine =
                                                    Vec::with_capacity(4 + response_data.len());
                                                combine.extend_from_slice(&response_length);
                                                combine.extend_from_slice(&response_data);

                                                match writer.write_all(&combine).await {
                                                    Ok(_) => match writer.flush().await {
                                                        Ok(_) => println!(
                                                            "📤 [Docker {}] 已发送响应到 {}",
                                                            server_id, client_addr
                                                        ),
                                                        Err(e) => println!(
                                                            "❌ [Docker {}] 刷新响应失败: {}",
                                                            server_id, e
                                                        ),
                                                    },
                                                    Err(e) => println!(
                                                        "❌ [Docker {}] 发送响应失败: {}",
                                                        server_id, e
                                                    ),
                                                }
                                            }
                                            Err(e) => println!(
                                                "❌ [Docker {}] 解析消息失败: {}",
                                                server_id, e
                                            ),
                                        }
                                    }
                                    Err(e) => {
                                        println!(
                                            "❌ [Docker {}] 读取消息内容失败: {}",
                                            server_id, e
                                        );
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                println!("❌ [Docker {}] 读取消息长度失败: {}", server_id, e);
                                break;
                            }
                        }
                    }

                    println!("🔌 [Docker {}] 连接已关闭: {}", server_id, client_addr);
                });
            }
            Err(e) => eprintln!("❌ [Docker {}] Connection failed: {}", server_id, e),
        }
    }
}

async fn start_node(ids: Vec<(SocketAddr, Uuid)>) -> Result<(), Box<dyn std::error::Error>> {
    let handle = tokio::runtime::Handle::current();

    // 创建客户端实例
    let mut server_builder = ComputerEngineWorker::new(handle.clone())?;

    // 初始连接到所有容器
    println!("🔌 [Node] 开始连接到所有容器...");
    for (id, remote) in ids.iter() {
        println!("🔄 [Node] 尝试连接到容器: {}", id);
        match server_builder.connect(remote.clone(), id.clone()).await {
            Ok(_) => println!("✅ [Node] 成功连接到容器: {}", id),
            Err(e) => eprintln!("❌ [Node] 连接到容器 {} 失败: {}", id, e),
        }
    }

    let server = server_builder.service().clone();
    handle.spawn(server_builder.run());

    println!("📝 [Node] 开始持续发送消息...");

    // 持续发送消息
    let mut counter = 1;
    loop {
        // ComputerEngineWorker现在会自动处理重连

        // 轮流向每个容器发送消息
        for (id, remote) in ids.iter() {
            // 构建消息内容
            let message = format!("消息 #{} 发送到 {}", counter, id);
            let res = server
                .request(Uuid::new_v4(), remote, serde_json::json!(message), None)
                .await
                .unwrap();
            println!("msg: {:?}", res);
            // // 发送消息并处理错误

            // 等待一段时间再发送下一条消息
            sleep(Duration::from_secs(2)).await;
        }

        counter += 1;
        // 每轮消息之间稍作等待
        sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_dir = "/tmp/test_unix_server";
    if !Path::new(socket_dir).exists() {
        std::fs::create_dir_all(socket_dir)?;
    }
    let ids = 3;
    let mut server_ids = Vec::new();
    for i in 1..=ids {
        let addr = format!("127.0.0.1:301{}", i).parse::<SocketAddr>().unwrap();
        server_ids.push((addr, Uuid::new_v4()));
    }

    let mut tasks = vec![];
    let mut i = 1;
    for path in server_ids.iter() {
        let path = path.clone();
        tasks.push(tokio::spawn(start_docker(i, path.0)));
        i += 1;
    }

    let server_task = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        if let Err(e) = start_node(server_ids).await {
            eprintln!("❌ [Node] Error: {}", e);
        }
    });

    // 保持程序运行，直到收到终止信号
    println!("程序正在运行，按Ctrl+C终止...");
    match signal::ctrl_c().await {
        Ok(()) => {
            println!("收到终止信号，正在关闭...");
            tasks.iter().for_each(|task| task.abort());
            server_task.abort();
        }
        Err(err) => {
            eprintln!("无法监听Ctrl+C信号: {}", err);
        }
    }

    let sock_path = Path::new(socket_dir);

    // 清理套接字目录
    if sock_path.exists() {
        let _ = std::fs::remove_dir_all(sock_path);
    }

    Ok(())
}
