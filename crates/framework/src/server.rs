use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::http::{HttpMethod, HttpRequest, HttpResponse};
use serde_json::json;
use std::convert::Infallible;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use uuid;

use crate::context::Context;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::middleware::{create_standard_middleware_chain, RouteHandler as MiddlewareRouteHandler};
use crate::routes::{Route, RouteDefinition, Router};
use crate::state::Contract;

/// Application configuration options
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Port to listen on
    pub port: u16,
    /// Host to bind to
    pub host: String,
    /// Database connection string
    pub database_url: Option<String>,
    /// Log level
    pub log_level: String,
}

impl AppConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            port: 8080,
            host: "127.0.0.1".to_string(),
            database_url: None,
            log_level: "info".to_string(),
        }
    }

    /// Set the port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the host
    pub fn with_host(mut self, host: &str) -> Self {
        self.host = host.to_string();
        self
    }

    /// Set the database URL
    pub fn with_database_url(mut self, url: &str) -> Self {
        self.database_url = Some(url.to_string());
        self
    }

    /// Set the log level
    pub fn with_log_level(mut self, level: &str) -> Self {
        self.log_level = level.to_string();
        self
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Modern Web2-style application for smart contract execution
pub struct App {
    router: Router,
    config: AppConfig,
    database: Arc<Database>,
    enable_state_tracking: bool,
    run_in_container: bool,
}

impl App {
    /// Create a new application with default configuration
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            config: AppConfig::default(),
            database: Arc::new(Database::new()),
            enable_state_tracking: true,
            run_in_container: false,
        }
    }

    /// Create a new application with specific configuration
    pub fn with_config(mut self, config: AppConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the database
    pub fn with_database(mut self, database: Database) -> Self {
        self.database = Arc::new(database);
        self
    }

    /// Enable state tracking for this application (enabled by default)
    pub fn enable_state_tracking(mut self, enable: bool) -> Self {
        self.enable_state_tracking = enable;
        self
    }

    /// Enable container execution for this application (enabled by default)
    pub fn run_in_container(mut self, enable: bool) -> Self {
        self.run_in_container = enable;
        self
    }

    /// Register routes
    pub fn register_routes(mut self, routes: Vec<RouteDefinition>) -> Self {
        self.router.add_routes(routes);
        self
    }

    /// Register a contract with this application
    pub fn register_contract<C: Contract + 'static>(mut self, contract: C) -> Self {
        let routes = contract.register_routes();
        self.router.add_routes(routes);
        self
    }

    /// Run the application on the specified address or default from config
    pub async fn run(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let addr: SocketAddr = addr
            .parse()
            .map_err(|e: std::net::AddrParseError| Error::ValidationError(e.to_string()))?;

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| Error::IoError(e.to_string()))?;
        println!("Server running on http://{}", addr);

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| Error::IoError(e.to_string()))?;
            let router = self.router.clone();
            let database = self.database.clone();
            let enable_state_tracking = self.enable_state_tracking;

            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, router, database, enable_state_tracking).await
                {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }
}

// 处理连接
async fn handle_connection(
    mut stream: TcpStream,
    router: Router,
    database: Arc<Database>,
    enable_state_tracking: bool,
) -> Result<()> {
    // 解析HTTP请求
    let request = HttpRequest::from_stream(&mut stream).await?;

    // 打印请求信息
    println!("收到请求: {} {}", request.method, request.path);

    // 创建事务ID
    let transaction_id = uuid::Uuid::new_v4().to_string();

    // 创建上下文
    let mut context = Context::with_database(Arc::new(database.as_ref().clone()));

    // 启用状态追踪
    if enable_state_tracking {
        context.enable_state_tracking();

        // 设置事务ID
        context.set_transaction_id(&transaction_id);
        println!("设置事务ID: {}", transaction_id);
    }

    // 查找路由处理函数
    let key = format!("{} {}", request.method, request.path);
    println!("查找路由: {}", key);

    // 打印所有已注册的路由
    println!("已注册的路由:");
    for route_key in router.get_routes() {
        println!("  {}", route_key);
    }

    let mut response = match router.get_handler(request.method, &request.path) {
        Some(handler) => {
            println!("找到路由处理程序");
            // 调用处理函数
            let resp = handler(request.clone(), context.clone()).await?;

            // 如果启用了状态追踪，获取状态差异并添加到响应中
            if enable_state_tracking {
                let state_diff = context.get_diff();
                println!("状态差异: {:?}", state_diff);

                // 创建新的响应，包含状态差异和事务ID
                let mut new_resp = resp.clone();
                new_resp.state_diff = Some(state_diff);
                new_resp.transaction_id = Some(context.transaction_id.clone());
                new_resp
            } else {
                resp
            }
        }
        None => {
            println!("未找到路由处理程序");
            // 未找到路由
            HttpResponse {
                status: 404,
                data: Some(serde_json::json!({"error": "Route not found"})),
                error: Some("Route not found".to_string()),
                transaction_id: None,
                state_diff: None,
            }
        }
    };

    // 发送响应
    response.send(&mut stream).await?;

    Ok(())
}

// 创建标准应用
pub fn create_app() -> App {
    App::new()
}

// 创建带有标准中间件的路由处理函数
pub fn create_handler_with_middleware(
    router: Arc<Router>,
    database: Arc<Database>,
) -> impl Fn(
    hyper::Request<hyper::Body>,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<hyper::Response<hyper::Body>>> + Send>,
> + Send
       + Sync
       + 'static {
    move |req| {
        let router = router.clone();
        let database = database.clone();
        Box::pin(async move { handle_request(req, router, database).await })
    }
}

/// 处理HTTP请求
async fn handle_request(
    req: hyper::Request<hyper::Body>,
    router: Arc<Router>,
    database: Arc<Database>,
) -> Result<hyper::Response<hyper::Body>> {
    // 解析请求
    let (parts, body) = req.into_parts();
    let body_bytes = hyper::body::to_bytes(body).await.unwrap_or_default();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

    // 创建事务ID
    let transaction_id = uuid::Uuid::new_v4().to_string();

    // 创建上下文
    let mut context = Context::with_database(Arc::new(database.as_ref().clone()));

    // 启用状态跟踪
    context.enable_state_tracking();

    // 设置事务ID
    context.set_transaction_id(&transaction_id);

    // 创建HTTP请求
    let http_request = HttpRequest {
        method: match parts.method {
            hyper::Method::GET => HttpMethod::Get,
            hyper::Method::POST => HttpMethod::Post,
            hyper::Method::PUT => HttpMethod::Put,
            hyper::Method::DELETE => HttpMethod::Delete,
            hyper::Method::OPTIONS => HttpMethod::Options,
            hyper::Method::HEAD => HttpMethod::Head,
            _ => HttpMethod::Get,
        },
        path: parts.uri.path().to_string(),
        headers: parts
            .headers
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect(),
        query: parse_query(&parts.uri),
        body: body_str,
    };

    // 查找路由
    if let Some(handler) = router.get_handler(http_request.method, &http_request.path) {
        // 处理请求
        match handler(http_request.clone(), context.clone()).await {
            Ok(response) => {
                // 创建响应
                let mut http_response = response;
                http_response.transaction_id = Some(transaction_id);

                // 序列化响应
                let json = serde_json::to_string(&http_response).unwrap_or_else(|_| {
                    String::from("{\"error\":\"Failed to serialize response\"}")
                });

                // 返回响应
                Ok(hyper::Response::builder()
                    .status(http_response.status)
                    .header("Content-Type", "application/json")
                    .body(hyper::Body::from(json))
                    .unwrap_or_else(|_| {
                        hyper::Response::builder()
                            .status(500)
                            .body(hyper::Body::from(
                                "{\"error\":\"Failed to build response\"}",
                            ))
                            .unwrap()
                    }))
            }
            Err(e) => {
                // 处理错误
                let error_response = HttpResponse::<serde_json::Value>::bad_request(&e.to_string())
                    .with_transaction_id(&transaction_id);

                let json = serde_json::to_string(&error_response).unwrap_or_else(|_| {
                    String::from("{\"error\":\"Failed to serialize error response\"}")
                });

                Ok(hyper::Response::builder()
                    .status(400)
                    .header("Content-Type", "application/json")
                    .body(hyper::Body::from(json))
                    .unwrap_or_else(|_| {
                        hyper::Response::builder()
                            .status(500)
                            .body(hyper::Body::from(
                                "{\"error\":\"Failed to build error response\"}",
                            ))
                            .unwrap()
                    }))
            }
        }
    } else {
        // 路由未找到
        let error_response = HttpResponse::<serde_json::Value>::not_found("Route not found")
            .with_transaction_id(&transaction_id);

        let json = serde_json::to_string(&error_response)
            .unwrap_or_else(|_| String::from("{\"error\":\"Route not found\"}"));

        Ok(hyper::Response::builder()
            .status(404)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(json))
            .unwrap_or_else(|_| {
                hyper::Response::builder()
                    .status(500)
                    .body(hyper::Body::from(
                        "{\"error\":\"Failed to build not found response\"}",
                    ))
                    .unwrap()
            }))
    }
}

// 解析查询参数
fn parse_query(uri: &hyper::Uri) -> HashMap<String, String> {
    let mut query_params = HashMap::new();

    if let Some(query) = uri.query() {
        for pair in query.split('&') {
            if let Some(eq_pos) = pair.find('=') {
                let key = pair[..eq_pos].to_string();
                let value = pair[eq_pos + 1..].to_string();
                query_params.insert(key, value);
            }
        }
    }

    query_params
}

/// Web服务器
pub struct Server {
    config: AppConfig,
    router: Option<axum::Router>,
}

impl Server {
    /// 创建新的服务器
    pub fn new() -> Self {
        Self {
            config: AppConfig::default(),
            router: None,
        }
    }

    /// 设置路由
    pub fn with_router(mut self, router: axum::Router) -> Self {
        self.router = Some(router);
        self
    }

    /// 启动服务器
    pub async fn start(self, addr: &str) -> Result<()> {
        let addr = addr
            .parse()
            .map_err(|e| Error::InternalError(format!("Failed to parse address: {}", e)))?;

        let router = self
            .router
            .ok_or_else(|| Error::InternalError("Router not set".to_string()))?;

        println!("Starting server on {}", addr);

        axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .await
            .map_err(|e| Error::InternalError(format!("Server error: {}", e)))?;

        Ok(())
    }
}
