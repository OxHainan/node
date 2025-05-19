// mp Framework - 为智能合约提供类似Web2的开发体验

// 导出所有模块
pub mod adapters;
pub mod context;
pub mod database;
pub mod errors;
pub mod http;
pub mod macros;
pub mod middleware;
pub mod model;
pub mod prelude;
pub mod routes;
pub mod server;
pub mod state;
pub mod state_serialize; // 新增适配器模块

// 重新导出常用组件
pub use context::Context;
pub use errors::{Error, Result};
pub use middleware::{
    create_standard_middleware_chain, logging_middleware, state_tracking_middleware,
    transaction_middleware,
};
pub use model::{HasMany, Model};
pub use routes::{create_router, delete, get, post, put, RouteDefinition, RouteType, Router};
pub use server::{create_app, create_handler_with_middleware, AppConfig};
pub use state::Contract;
pub use state_serialize::{
    Identifiable, ModelUtils, QuerySupport, StateContextExt, StateSerializable,
};

// 重新导出适配器模块
pub use adapters::DatabaseProxy;

// 重新导出派生宏
pub use mp_derive::*;

// 容器内执行架构，合约在容器内使用内存数据库执行，
// 产生的状态变化由框架自动跟踪，并在执行完成后通过主动通知系统返回给节点
