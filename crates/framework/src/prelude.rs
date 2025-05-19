// prelude.rs - 导出所有常用组件，简化导入

// 导出框架核心组件
pub use crate::context::Context;
pub use crate::http::{HttpMethod, HttpRequest, HttpResponse};
pub use crate::server::AppConfig;

// 导出路由相关组件
pub use crate::routes::{create_router, delete, get, post, put};
pub use crate::routes::{Route, RouteDefinition, RouteType, Router};

// 导出中间件相关组件
pub use crate::middleware::{
    create_standard_middleware_chain, logging_middleware, state_tracking_middleware,
    transaction_middleware, MiddlewareBuilder, RouteHandler,
};

// 导出数据库相关组件
pub use crate::database::{Database, Transaction};

// 导出状态相关组件
pub use crate::state::{Contract, StateDiff, StateTracker};
pub use crate::state_serialize::{
    Identifiable, ModelUtils, QuerySupport, StateContextExt, StateSerializable,
};

// 导出模型相关组件
pub use crate::model::{HasMany, Model};

// 导出错误类型
pub use crate::errors::{Error, Result};

// 导出常用宏
pub use crate::{impl_model, impl_state_entity};
