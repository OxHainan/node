use futures::{channel::mpsc, StreamExt};
use log::error;
use request_response::RequestResponses;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::{EngineService, RequestFailure, ServiceToWorkerMsg};
use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    runtime::Handle,
    sync::Mutex,
};

use uuid::Uuid;
mod event;
mod out_events;
mod request_response;
pub mod service;

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum Status {
    STOP,
    RUNNING,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum Method {
    Invoke,
    Setstate,
    Getstate,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Params {
    pub id: Uuid,
    pub method: Method,
    pub status: Status,
    pub data: Value,
}

// 将 ComputerEngine 拆分为读写两部分
pub struct ComputerEngineReader {
    reader: Arc<Mutex<ReadHalf<TcpStream>>>,
    to_local: mpsc::UnboundedSender<ReceiverMessage>,
    remote: Uuid,
    container_id: SocketAddr,
    // 标记引擎是否正在运行
    running: Arc<AtomicBool>,
}

impl ComputerEngineReader {
    async fn run(mut self) {
        while let Some(params) = self.read_message().await {
            let _ = self.to_local.unbounded_send(ReceiverMessage {
                remote: self.remote,
                params,
            });
        }
    }
}

impl ComputerEngineReader {
    // 新的异步函数，用于读取消息
    pub async fn read_message(&mut self) -> Option<Params> {
        use tokio::io::AsyncReadExt;

        // 检查引擎是否正在运行
        if !self.running.load(Ordering::SeqCst) {
            return None;
        }

        // 尝试获取读取锁
        let mut reader_guard = self.reader.lock().await;

        // 读取消息长度（4字节）
        let mut length_buffer = [0u8; 4];
        match reader_guard.read_exact(&mut length_buffer).await {
            Ok(_) => {
                // 成功读取长度
                let length = u32::from_le_bytes(length_buffer) as usize;

                // 读取消息内容
                let mut data_buffer = vec![0u8; length];
                match reader_guard.read_exact(&mut data_buffer).await {
                    Ok(_) => {
                        // 成功读取数据，尝试解析
                        match serde_json::from_slice::<Params>(&data_buffer) {
                            Ok(params) => Some(params),
                            Err(e) => {
                                error!("解析数据失败: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() != io::ErrorKind::WouldBlock {
                            error!("读取消息内容失败: {}", e);
                        }
                        None
                    }
                }
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    error!("读取消息长度失败: {}", e);
                }
                None
            }
        }
    }
}

pub struct ComputerEngineWriter {
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    container_id: SocketAddr,
    // 标记引擎是否正在运行
    running: Arc<AtomicBool>,
}

impl ComputerEngineWriter {
    /// 检查连接是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    async fn send(&mut self, params: Params) -> io::Result<()> {
        // 检查连接是否还在运行
        if !self.running.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "连接已关闭"));
        }

        // 尝试序列化数据
        let data = match serde_json::to_vec(&params) {
            Ok(data) => data,
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("序列化数据失败: {}", e),
                ))
            }
        };

        // 准备发送数据
        let length = (data.len() as u32).to_le_bytes();
        let mut combine = Vec::with_capacity(4 + data.len());
        combine.extend_from_slice(&length);
        combine.extend_from_slice(&data);

        // 获取写入器锁
        let mut writer = match self.writer.try_lock() {
            Ok(guard) => guard,
            Err(_) => self.writer.lock().await,
        };

        // 尝试写入数据
        match writer.write_all(&combine).await {
            Ok(_) => {
                // 写入成功，刷新缓冲区
                match writer.flush().await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        // 如果是管道断开错误，标记连接已关闭
                        if e.kind() == io::ErrorKind::BrokenPipe {
                            self.running.store(false, Ordering::SeqCst);
                        }
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // 如果是管道断开错误，标记连接已关闭
                if e.kind() == io::ErrorKind::BrokenPipe {
                    self.running.store(false, Ordering::SeqCst);
                }
                Err(e)
            }
        }
    }
}

pub struct ReceiverMessage {
    pub remote: Uuid,
    pub params: Params,
}

pub struct ComputerEngineWorker {
    connections: HashMap<Uuid, ComputerEngineWriter>,
    executor: Handle,
    // 存储所有容器的from_local通道
    from_locals: mpsc::UnboundedReceiver<ReceiverMessage>,
    to_locals: mpsc::UnboundedSender<ReceiverMessage>,
    from_service: mpsc::UnboundedReceiver<ServiceToWorkerMsg>,
    service: Arc<EngineService>,
    request_responses: RequestResponses,
    // 自动重连标志
    auto_reconnect: bool,
    // 最大重连次数
    max_reconnect_attempts: u32,
    // 重连间隔（毫秒）
    reconnect_interval_ms: u64,
    event_streams: out_events::Channels,
}

impl ComputerEngineWorker {
    pub fn new(executor: Handle) -> io::Result<Self> {
        let (to_worker, from_service) = mpsc::unbounded();
        let (to_locals, from_locals) = mpsc::unbounded();
        let service = Arc::new(EngineService::new(to_worker));
        Ok(Self {
            connections: HashMap::new(),
            executor,
            from_locals,
            to_locals,
            from_service,
            service,
            auto_reconnect: true,        // 默认启用自动重连
            max_reconnect_attempts: 5,   // 默认最大重连次数
            reconnect_interval_ms: 1000, // 默认重连间隔为1秒
            event_streams: out_events::Channels::new(),
            request_responses: RequestResponses::new(),
        })
    }

    pub async fn handle_wortker_message(&mut self, msg: ServiceToWorkerMsg) {
        match msg {
            ServiceToWorkerMsg::EventStream(sender) => {
                self.event_streams.push(sender);
            }
            ServiceToWorkerMsg::Request {
                request_id,
                remote,
                request,
                fallback_request,
                pending_response,
            } => {
                if let Some(engine) = self.get_engine_mut(&remote) {
                    engine
                        .send(Params {
                            id: request_id,
                            method: Method::Invoke,
                            status: Status::RUNNING,
                            data: request,
                        })
                        .await
                        .unwrap();
                    self.request_responses.pending_requests(
                        request_id,
                        fallback_request,
                        pending_response,
                    );
                } else {
                    pending_response
                        .send(Err(RequestFailure::NotConnected))
                        .unwrap();
                }
            }
        }
    }

    pub fn service(&self) -> &Arc<EngineService> {
        &self.service
    }

    /// 设置自动重连选项
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// 设置最大重连次数
    pub fn with_max_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.max_reconnect_attempts = attempts;
        self
    }

    /// 设置重连间隔（毫秒）
    pub fn with_reconnect_interval(mut self, interval_ms: u64) -> Self {
        self.reconnect_interval_ms = interval_ms;
        self
    }

    /// 连接到指定容器ID的服务
    fn connect_to_container(
        &mut self,
        stream: TcpStream,
        remote: Uuid,
        container_id: SocketAddr,
    ) -> io::Result<()> {
        let (reader_half, writer_half) = tokio::io::split(stream);
        let reader = Arc::new(Mutex::new(reader_half));
        let writer = Arc::new(Mutex::new(writer_half));
        let runner = Arc::new(AtomicBool::new(true));

        let reader_engine = ComputerEngineReader {
            reader,
            container_id,
            remote,
            running: runner.clone(),
            to_local: self.to_locals.clone(),
        };

        let writer_engine = ComputerEngineWriter {
            writer,
            container_id,
            running: runner.clone(),
        };

        self.connections.insert(remote, writer_engine);
        self.executor.spawn(reader_engine.run());

        Ok(())
    }

    fn handle_accept(
        &mut self,
        stream: TcpStream,
        remote: Uuid,
        addr: SocketAddr,
    ) -> io::Result<()> {
        self.connect_to_container(stream, remote, addr)?;
        Ok(())
    }

    /// 连接到指定容器
    pub async fn connect(&mut self, remote: Uuid, container_id: SocketAddr) -> io::Result<()> {
        println!("连接到容器: {:?}", container_id);

        // 连接到容器
        let stream = TcpStream::connect(container_id).await?;
        self.handle_accept(stream, remote, container_id)?;
        Ok(())
    }

    /// 重连到指定容器
    async fn reconnect(&mut self, remote: &Uuid) -> io::Result<()> {
        // 先删除旧的连接
        let Some(engine) = self.connections.remove(&remote) else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("移除 {} 失败", remote),
            ));
        };

        // 尝试重连指定次数
        let mut attempts = 0;
        let max_attempts = self.max_reconnect_attempts;
        let interval = self.reconnect_interval_ms;

        let container_id = engine.container_id;

        while attempts < max_attempts {
            attempts += 1;

            match TcpStream::connect(container_id).await {
                Ok(stream) => {
                    return self.handle_accept(stream, *remote, container_id);
                }
                Err(e) => {
                    if attempts < max_attempts {
                        // 等待一段时间再尝试
                        tokio::time::sleep(tokio::time::Duration::from_millis(interval)).await;
                    }
                }
            }
        }

        // 所有重连尝试失败
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            format!(
                "重连到容器 {} 失败，已尝试 {} 次",
                container_id, max_attempts
            ),
        ))
    }

    /// 获取所有已连接的容器ID列表
    pub fn get_connected_containers(&self) -> Vec<Uuid> {
        self.connections.keys().cloned().collect()
    }

    /// 获取指定容器的引擎
    pub fn get_engine(&self, remote: &Uuid) -> Option<&ComputerEngineWriter> {
        self.connections.get(remote)
    }

    /// 获取指定容器的引擎的可变引用
    pub fn get_engine_mut(&mut self, remote: &Uuid) -> Option<&mut ComputerEngineWriter> {
        self.connections.get_mut(remote)
    }

    pub async fn next_action(&mut self) -> bool {
        futures::select! {
            receiver = self.from_locals.select_next_some() => {
                self.handle_remote_message(receiver);
            }
            msg = self.from_service.select_next_some() => {
                self.handle_wortker_message(msg).await;
            }
        }

        true
    }

    // 处理从远程接收的消息
    fn handle_remote_message(&mut self, msg: ReceiverMessage) {
        self.request_responses.on_request_response(msg.params);
    }

    pub async fn run(mut self) {
        while self.next_action().await {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试创建ComputerEngineWorker
    #[tokio::test]
    async fn test_create_builder() {
        // 使用当前测试函数的运行时句柄
        let handle = tokio::runtime::Handle::current();

        let builder = ComputerEngineWorker::new(handle).unwrap();
        assert!(builder.connections.is_empty());
    }
}
