use crate::context::Context;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::model::Model;
use crate::state::StateDiff;
use crate::state_serialize::{Identifiable, StateSerializable};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 模型适配器 - 为现有模型提供状态追踪能力
pub struct ModelAdapter<T: Clone> {
    // 原始模型
    model: T,
    // 实体类型
    entity_type: &'static str,
    // 表名
    table_name: &'static str,
    // ID提取器
    id_extractor: Box<dyn Fn(&T) -> String + Send + Sync>,
    // 序列化器
    serializer: Box<dyn Fn(&T) -> Vec<(String, String)> + Send + Sync>,
}

impl<T: Clone> ModelAdapter<T> {
    /// 创建新的适配器
    pub fn new(model: T) -> Self {
        Self {
            model,
            entity_type: "Unknown",
            table_name: "unknown",
            id_extractor: Box::new(|_| String::new()),
            serializer: Box::new(|_| Vec::new()),
        }
    }

    /// 设置实体类型
    pub fn with_entity_type(mut self, entity_type: &'static str) -> Self {
        self.entity_type = entity_type;
        self
    }

    /// 设置表名
    pub fn with_table_name(mut self, table_name: &'static str) -> Self {
        self.table_name = table_name;
        self
    }

    /// 设置ID提取器
    pub fn with_id_extractor<F>(mut self, extractor: F) -> Self
    where
        F: Fn(&T) -> String + Send + Sync + 'static,
    {
        self.id_extractor = Box::new(extractor);
        self
    }

    /// 设置序列化器
    pub fn with_serializer<F>(mut self, serializer: F) -> Self
    where
        F: Fn(&T) -> Vec<(String, String)> + Send + Sync + 'static,
    {
        self.serializer = Box::new(serializer);
        self
    }

    /// 获取实体ID
    pub fn get_id(&self) -> String {
        (self.id_extractor)(&self.model)
    }

    /// 获取实体类型
    pub fn get_entity_type(&self) -> &'static str {
        self.entity_type
    }

    /// 获取表名
    pub fn get_table_name(&self) -> &'static str {
        self.table_name
    }

    /// 序列化为状态条目
    pub fn to_state_entries(&self) -> Vec<(String, String)> {
        (self.serializer)(&self.model)
    }

    /// 获取原始模型
    pub fn model(&self) -> &T {
        &self.model
    }

    /// 获取可变原始模型
    pub fn model_mut(&mut self) -> &mut T {
        &mut self.model
    }
}

/// 上下文扩展 - 为Context添加适配器支持
pub trait ContextExt {
    /// 使用适配器跟踪实体
    fn track_entity_with_adapter<T: Clone>(&mut self, adapter: &ModelAdapter<T>) -> Result<()>;

    /// 自动事务管理
    fn with_transaction<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Self) -> Result<R>;
}

impl ContextExt for Context {
    fn track_entity_with_adapter<T: Clone>(&mut self, adapter: &ModelAdapter<T>) -> Result<()> {
        // 获取实体ID和类型
        let entity_id = adapter.get_id();
        let entity_type = adapter.get_entity_type();

        // 获取状态条目
        let entries = adapter.to_state_entries();

        // 记录状态变更
        for (key, value) in entries.iter() {
            let state_key = format!("{}/{}/{}", entity_type, entity_id, key);
            self.set_state_key(&state_key, value);
        }

        // 记录实体变更 - 使用现有的track_entity方法
        let json_value =
            serde_json::to_value(&entries).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 创建一个临时实体来跟踪变更
        let entity = JsonEntity {
            entity_type: entity_type.to_string(),
            id: entity_id.to_string(),
            data: json_value,
        };

        // 使用 track_entity 方法
        self.track_entity(&entity)?;

        Ok(())
    }

    fn with_transaction<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Self) -> Result<R>,
    {
        // 启用状态跟踪
        self.enable_state_tracking();

        // 执行函数
        let result = f(self)?;

        // 获取状态差异（但不结束事务）
        let _diff = self.get_diff();

        Ok(result)
    }
}

/// 数据库代理 - 简化数据库访问和自动状态追踪
pub struct DatabaseProxy<'a> {
    db: &'a Database,
    ctx: &'a mut Context,
}

impl<'a> DatabaseProxy<'a> {
    /// 创建新的数据库代理
    pub fn new(db: &'a Database, ctx: &'a mut Context) -> Self {
        Self { db, ctx }
    }

    /// 保存模型实体 - 自动跟踪状态
    pub fn save<T>(&mut self, entity: &T) -> Result<()>
    where
        T: Model + StateSerializable + Identifiable + Serialize,
    {
        // 获取实体ID和表名
        let id = <T as Identifiable>::get_id(entity);
        let table = <T as Model>::get_table_name();
        let entity_type = <T as Model>::get_entity_type();

        // 序列化实体
        let json =
            serde_json::to_string(entity).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 保存到数据库
        self.db.insert(table, &id, &json)?;

        // 自动跟踪状态变更 - 直接注册键值对
        if let Ok(entity_json) = serde_json::to_value(entity) {
            if let Some(obj) = entity_json.as_object() {
                // 为每个字段生成键值对并自动记录状态变更
                for (key, value) in obj {
                    let state_key = format!("{}/{}/{}", entity_type, id, key);
                    let value_str = value.to_string().trim_matches('"').to_string();
                    self.ctx.set_state_key(&state_key, &value_str);
                }
            }
        }

        // 跟踪实体变更
        self.ctx.track_entity::<T>(entity)?;

        Ok(())
    }

    /// 查询实体
    pub fn get<T>(&self, id: &str) -> Result<Option<T>>
    where
        T: Model + for<'de> Deserialize<'de>,
    {
        self.find::<T>(T::get_table_name(), id)
    }

    /// 查询所有实体
    pub fn get_all<T>(&self) -> Result<Vec<T>>
    where
        T: Model + for<'de> Deserialize<'de>,
    {
        self.find_all::<T>(T::get_table_name())
    }

    /// 查询实体
    pub fn find<T: for<'de> Deserialize<'de>>(&self, table: &str, id: &str) -> Result<Option<T>> {
        if let Some(json) = self.db.get(table, id)? {
            let entity = serde_json::from_str(&json)
                .map_err(|e| Error::SerializationError(e.to_string()))?;
            Ok(Some(entity))
        } else {
            Ok(None)
        }
    }

    /// 查询所有实体
    pub fn find_all<T: for<'de> Deserialize<'de>>(&self, table: &str) -> Result<Vec<T>> {
        let mut entities = Vec::new();

        if let Some(table_data) = self.db.get_table(table)? {
            for row in table_data {
                if let Some(json) = row.get("data") {
                    let entity: T = serde_json::from_str(json)
                        .map_err(|e| Error::SerializationError(e.to_string()))?;
                    entities.push(entity);
                }
            }
        }

        Ok(entities)
    }

    /// 查询满足条件的实体
    pub fn find_where<T: for<'de> Deserialize<'de>, F>(
        &self,
        table: &str,
        predicate: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(&T) -> bool,
    {
        let mut entities = Vec::new();

        if let Some(table_data) = self.db.get_table(table)? {
            for row in table_data {
                if let Some(json) = row.get("data") {
                    let entity: T = serde_json::from_str(json)
                        .map_err(|e| Error::SerializationError(e.to_string()))?;

                    if predicate(&entity) {
                        entities.push(entity);
                    }
                }
            }
        }

        Ok(entities)
    }

    /// 删除实体
    pub fn delete<T>(&mut self, entity: &T) -> Result<()>
    where
        T: Model + Identifiable,
    {
        // 获取实体ID和表名
        let id = <T as Identifiable>::get_id(entity);
        let table = <T as Model>::get_table_name();

        // 从数据库删除
        self.db.delete(table, &id)?;

        // 记录删除操作
        self.ctx.delete_entity::<T>(&id)?;

        Ok(())
    }

    /// 删除指定ID的实体
    pub fn delete_by_id<T>(&mut self, id: &str) -> Result<()>
    where
        T: Model + Identifiable,
    {
        // 获取表名
        let table = T::get_table_name();

        // 从数据库删除
        self.db.delete(table, id)?;

        // 记录删除操作
        self.ctx.delete_entity::<T>(id)?;

        Ok(())
    }

    /// 批量保存实体
    pub fn batch_save<T>(&mut self, entities: &[T]) -> Result<()>
    where
        T: Model + StateSerializable + Identifiable + Serialize,
    {
        for entity in entities {
            self.save(entity)?;
        }

        Ok(())
    }
}

/// ORM适配器 - 为现有ORM提供状态追踪能力
pub struct OrmAdapter<'a> {
    db_proxy: DatabaseProxy<'a>,
}

impl<'a> OrmAdapter<'a> {
    /// 创建新的ORM适配器
    pub fn new(db: &'a Database, ctx: &'a mut Context) -> Self {
        Self {
            db_proxy: DatabaseProxy::new(db, ctx),
        }
    }

    /// 获取数据库代理
    pub fn db_proxy(&mut self) -> &mut DatabaseProxy<'a> {
        &mut self.db_proxy
    }
}

/// Web框架适配器 - 为现有Web框架提供状态追踪能力
pub struct WebFrameworkAdapter {
    routes: Vec<(
        String,
        String,
        Arc<dyn Fn(HttpRequest, Context) -> Result<HttpResponse> + Send + Sync>,
    )>,
}

// 简化的HTTP请求和响应类型
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: String,
}

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
    pub state_diff: Option<StateDiff>,
    pub transaction_id: Option<String>,
}

impl WebFrameworkAdapter {
    /// 创建新的Web框架适配器
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// 添加GET路由
    pub fn get<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(HttpRequest, Context) -> Result<HttpResponse> + Send + Sync + 'static,
    {
        let path = path.to_string();
        let method = "GET".to_string();
        let handler = Arc::new(handler);

        self.routes.push((method, path, handler));
        self
    }

    /// 添加POST路由
    pub fn post<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(HttpRequest, Context) -> Result<HttpResponse> + Send + Sync + 'static,
    {
        let path = path.to_string();
        let method = "POST".to_string();
        let handler = Arc::new(handler);

        self.routes.push((method, path, handler));
        self
    }
}

// 定义一个简单的 JSON 实体结构体，用于跟踪变更
#[derive(Serialize, Deserialize, Clone)]
struct JsonEntity {
    entity_type: String,
    id: String,
    data: serde_json::Value,
}

// 为 JsonEntity 实现必要的 trait
impl Identifiable for JsonEntity {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn get_entity_type() -> &'static str {
        "json_entity"
    }
}

impl StateSerializable for JsonEntity {
    fn save(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Model + Serialize,
    {
        ctx.track_entity(self)
    }

    fn delete(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Model,
    {
        ctx.delete_entity::<Self>(&<Self as Identifiable>::get_id(self))
    }
}

impl Model for JsonEntity {
    fn get_table_name() -> &'static str {
        "json_entity"
    }

    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| Error::DeserializationError(e.to_string()))
    }

    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| Error::SerializationError(e.to_string()))
    }
}
