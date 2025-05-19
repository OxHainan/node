use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::context::Context;
use crate::database::Database;
use crate::errors::Result;
use crate::http::{HttpRequest, HttpResponse};

/// 中间件类型 - 处理请求并返回响应或传递给下一个中间件
pub type Middleware = Arc<
    dyn Fn(
            HttpRequest,
            Context,
            Next,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

/// 下一个中间件类型
pub type Next = Arc<
    dyn Fn(
            HttpRequest,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

/// 路由处理函数类型
pub type RouteHandler = Arc<
    dyn Fn(
            HttpRequest,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

/// 中间件构建器 - 用于构建中间件链
pub struct MiddlewareBuilder {
    middlewares: Vec<Middleware>,
    handler: Option<RouteHandler>,
}

impl MiddlewareBuilder {
    /// 创建新的中间件构建器
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
            handler: None,
        }
    }

    /// 添加中间件
    pub fn use_middleware<F, Fut>(mut self, middleware: F) -> Self
    where
        F: Fn(HttpRequest, Context, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
    {
        self.middlewares.push(Arc::new(move |req, ctx, next| {
            let fut = middleware(req, ctx, next);
            Box::pin(fut)
                as Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
        }));
        self
    }

    /// 设置处理函数
    pub fn handler<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(HttpRequest, Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse<serde_json::Value>>> + Send + 'static,
    {
        self.handler = Some(Arc::new(move |req, ctx| {
            let fut = handler(req, ctx);
            Box::pin(fut)
                as Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
        }));
        self
    }

    /// 构建中间件链
    pub fn build(self) -> RouteHandler {
        let middlewares = self.middlewares;
        let handler = self.handler.unwrap_or_else(|| {
            Arc::new(|_, _| {
                Box::pin(async {
                    Ok(HttpResponse {
                        status: 404,
                        data: Some(serde_json::json!({"error": "Not found"})),
                        error: Some("Route not found".to_string()),
                        transaction_id: None,
                        state_diff: None,
                    })
                })
            })
        });

        Arc::new(move |req, ctx| {
            let mut chain: Vec<RouteHandler> = Vec::new();
            for middleware in middlewares.iter().rev() {
                let next = if chain.is_empty() {
                    let handler = handler.clone();
                    Arc::new(move |req, ctx| handler(req, ctx)) as Next
                } else {
                    let next = chain.last().unwrap().clone();
                    Arc::new(move |req, ctx| next(req, ctx)) as Next
                };

                let middleware = middleware.clone();
                chain
                    .push(Arc::new(move |req, ctx| middleware(req, ctx, next.clone()))
                        as RouteHandler);
            }

            let handler = if chain.is_empty() {
                handler.clone()
            } else {
                chain.last().unwrap().clone()
            };

            handler(req, ctx)
        })
    }
}

/// 状态跟踪中间件 - 启用状态跟踪并在响应中添加状态差异
pub fn state_tracking_middleware(
    req: HttpRequest,
    mut ctx: Context,
    next: Next,
) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>> {
    Box::pin(async move {
        // 启用状态跟踪
        ctx.enable_state_tracking();

        // 调用下一个中间件
        let response = next(req, ctx.clone()).await?;

        // 这里简化实现，不添加状态差异

        Ok(response)
    })
}

/// 事务中间件 - 为每个请求创建事务
pub fn transaction_middleware(
    req: HttpRequest,
    mut ctx: Context,
    next: Next,
) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>> {
    Box::pin(async move {
        // 生成事务ID
        let transaction_id = uuid::Uuid::new_v4().to_string();

        // 设置事务ID
        ctx.set_transaction_id(&transaction_id);

        // 调用下一个中间件
        let mut response = next(req, ctx.clone()).await?;

        // 添加事务ID到响应
        response.transaction_id = Some(transaction_id);

        Ok(response)
    })
}

/// 日志中间件 - 记录请求和响应
pub fn logging_middleware(
    req: HttpRequest,
    ctx: Context,
    next: Next,
) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>> {
    Box::pin(async move {
        // 记录请求
        println!("[INFO] Request: {} {}", req.method, req.path);

        // 调用下一个中间件或处理函数
        let response = next(req.clone(), ctx).await?;

        // 记录响应
        println!("[INFO] Response: {} {}", req.method, req.path);

        Ok(response)
    })
}

/// 上下文中间件 - 为每个请求创建上下文
pub fn context_middleware(
    db: Arc<Database>,
) -> impl Fn(
    HttpRequest,
    Next,
) -> Pin<Box<dyn Future<Output = Result<HttpResponse<serde_json::Value>>> + Send>>
       + Send
       + Sync
       + 'static {
    move |req, next| {
        let db = db.clone();
        Box::pin(async move {
            // 创建上下文
            let ctx = Context::with_database(db.clone());

            // 调用下一个中间件
            next(req, ctx).await
        })
    }
}

/// 创建标准中间件链
pub fn create_standard_middleware_chain(db: Arc<Database>, handler: RouteHandler) -> RouteHandler {
    MiddlewareBuilder::new()
        .use_middleware(move |req, _, next| {
            let db = db.clone();
            let context_middleware = context_middleware(db);
            context_middleware(req, next)
        })
        .use_middleware(transaction_middleware)
        .use_middleware(state_tracking_middleware)
        .use_middleware(logging_middleware)
        .handler(move |req, ctx| handler(req, ctx))
        .build()
}
