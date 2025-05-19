use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::context::Context;
use crate::errors::Result;
use crate::http::{HttpMethod, HttpRequest, HttpResponse};
use crate::middleware::RouteHandler as MiddlewareRouteHandler;

// 路由特性
pub trait Route {
    /// 注册路由
    fn register_routes(&self) -> Vec<RouteDefinition>;
}

// 路由类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteType {
    Get,
    Post,
    Put,
    Delete,
}

impl From<RouteType> for HttpMethod {
    fn from(route_type: RouteType) -> Self {
        match route_type {
            RouteType::Get => HttpMethod::Get,
            RouteType::Post => HttpMethod::Post,
            RouteType::Put => HttpMethod::Put,
            RouteType::Delete => HttpMethod::Delete,
        }
    }
}

impl RouteType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteType::Get => "GET",
            RouteType::Post => "POST",
            RouteType::Put => "PUT",
            RouteType::Delete => "DELETE",
        }
    }
}

// 路由处理函数类型
pub type RouteHandlerFn = Arc<
    dyn Fn(
            HttpRequest,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

// 路由定义
#[derive(Clone)]
pub struct RouteDefinition {
    pub path: String,
    pub method: RouteType,
    pub handler: RouteHandlerFn,
    pub middlewares:
        Vec<Arc<dyn Fn(MiddlewareRouteHandler) -> MiddlewareRouteHandler + Send + Sync>>,
}

impl RouteDefinition {
    pub fn new<F, Fut>(path: &str, method: RouteType, handler: F) -> Self
    where
        F: Fn(HttpRequest, Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
    {
        println!("注册路由: {} {}", method.as_str(), path);
        Self {
            path: path.to_string(),
            method,
            handler: Arc::new(move |req, ctx| {
                let fut = handler(req, ctx);
                Box::pin(fut)
                    as Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
            }),
            middlewares: Vec::new(),
        }
    }

    // 添加中间件
    pub fn with_middleware<M>(mut self, middleware: M) -> Self
    where
        M: Fn(MiddlewareRouteHandler) -> MiddlewareRouteHandler + Send + Sync + 'static,
    {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    // 构建处理函数
    pub fn build_handler(&self) -> MiddlewareRouteHandler {
        let handler = self.handler.clone();
        let base_handler: MiddlewareRouteHandler = Arc::new(move |req, ctx| handler(req, ctx));

        // 应用中间件
        let mut final_handler = base_handler;
        for middleware in self.middlewares.iter().rev() {
            final_handler = middleware(final_handler);
        }

        final_handler
    }
}

// 路由定义帮助函数
pub fn get<F, Fut>(path: &str, handler: F) -> RouteDefinition
where
    F: Fn(HttpRequest, Context) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
{
    RouteDefinition::new(path, RouteType::Get, handler)
}

pub fn post<F, Fut>(path: &str, handler: F) -> RouteDefinition
where
    F: Fn(HttpRequest, Context) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
{
    RouteDefinition::new(path, RouteType::Post, handler)
}

pub fn put<F, Fut>(path: &str, handler: F) -> RouteDefinition
where
    F: Fn(HttpRequest, Context) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
{
    RouteDefinition::new(path, RouteType::Put, handler)
}

pub fn delete<F, Fut>(path: &str, handler: F) -> RouteDefinition
where
    F: Fn(HttpRequest, Context) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
{
    RouteDefinition::new(path, RouteType::Delete, handler)
}

// 路由集合
#[derive(Clone)]
pub struct Router {
    routes: HashMap<String, MiddlewareRouteHandler>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn add_route(&mut self, route: RouteDefinition) {
        let key = format!("{} {}", route.method.as_str(), route.path);
        println!("添加路由: {}", key);
        let handler = route.build_handler();
        self.routes.insert(key, handler);
    }

    pub fn add_routes(&mut self, routes: Vec<RouteDefinition>) {
        for route in routes {
            self.add_route(route);
        }
    }

    pub fn get_handler(&self, method: HttpMethod, path: &str) -> Option<&MiddlewareRouteHandler> {
        // 使用HttpMethod的Display实现来获取字符串表示
        let method_str = method.to_string();
        let key = format!("{} {}", method_str, path);
        println!("尝试查找路由: {}", key);
        self.routes.get(&key)
    }

    // 获取所有已注册的路由
    pub fn get_routes(&self) -> Vec<String> {
        self.routes.keys().cloned().collect()
    }
}

// 创建路由集合
pub fn create_router(routes: Vec<RouteDefinition>) -> Router {
    let mut router = Router::new();
    router.add_routes(routes);
    router
}
