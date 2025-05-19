use crate::context::{Context, EntityDiff};
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::model::Model;
use crate::state::StateDiff;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

/// 可标识的实体特性
pub trait Identifiable {
    /// 获取实体ID
    fn get_id(&self) -> String;

    /// 获取实体类型
    fn get_entity_type() -> &'static str
    where
        Self: Sized;
}

/// 状态序列化特性
pub trait StateSerializable: Identifiable {
    /// 序列化为JSON
    fn to_json(&self) -> Result<String>
    where
        Self: Serialize,
    {
        serde_json::to_string(self).map_err(|e| Error::SerializationError(e.to_string()))
    }

    /// 从JSON反序列化
    fn from_json(json: &str) -> Result<Self>
    where
        Self: Sized + for<'de> Deserialize<'de>,
    {
        serde_json::from_str(json).map_err(|e| Error::SerializationError(e.to_string()))
    }

    /// 保存实体
    fn save(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Model + Serialize;

    /// 删除实体
    fn delete(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Model;

    /// 获取状态差异
    fn get_state_diff(&self, ctx: &Context) -> Result<serde_json::Value>
    where
        Self: Sized,
    {
        Ok(serde_json::json!({
            "entity_type": <Self as Identifiable>::get_entity_type(),
            "id": <Self as Identifiable>::get_id(self),
            "state_diffs": ctx.state_diffs,
            "entity_diffs": ctx.entity_diffs,
        }))
    }
}

/// 实体查询支持
pub trait QuerySupport {
    /// 查询实体
    fn query_entity<T: StateSerializable + Identifiable + for<'de> Deserialize<'de>>(
        &self,
        id: &str,
    ) -> Result<Option<T>>;

    /// 查询实体列表
    fn query_entities<T: StateSerializable + Identifiable + for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        params: &[&str],
    ) -> Result<Vec<T>>;
}

/// 结果扩展
pub trait ResultExt<T> {
    /// 实体未找到错误
    fn entity_not_found(self, entity_type: &str, id: &str) -> Result<T>;
}

impl<T> ResultExt<T> for Result<Option<T>> {
    fn entity_not_found(self, entity_type: &str, id: &str) -> Result<T> {
        match self {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(Error::NotFound(format!(
                "{} with ID {} not found",
                entity_type, id
            ))),
            Err(e) => Err(e),
        }
    }
}

/// 实体保存特性
pub trait EntitySave<T: StateSerializable + Identifiable + Serialize> {
    /// 保存实体
    fn save_entity(&self, ctx: &mut Context, entity: &T) -> Result<()>;
}

/// 实体删除特性
pub trait EntityDelete<T: Identifiable> {
    /// 删除实体
    fn delete_entity(&self, ctx: &mut Context, id: &str) -> Result<()>;
}

/// 实体查询特性
pub trait EntityQuery<T: StateSerializable + Identifiable + for<'de> Deserialize<'de>> {
    /// 查询实体
    fn query_entity(&self, ctx: &Context, id: &str) -> Result<Option<T>>;

    /// 查询实体列表
    fn query_entities(
        &self,
        ctx: &Context,
        query: &str,
        params: &[&dyn erased_serde::Serialize],
    ) -> Result<Vec<T>>;
}

/// 模型查询扩展
pub trait ModelQueryExt<T: Model + for<'de> Deserialize<'de>> {
    /// 查询实体
    fn find(id: &str) -> Result<Option<T>>
    where
        T: StateSerializable + Identifiable;

    /// 查询实体（如果不存在则返回错误）
    fn find_or_error(id: &str) -> Result<T>
    where
        T: StateSerializable + Identifiable;

    /// 查询实体（带上下文）
    fn find_with_context(ctx: &Context, id: &str) -> Result<Option<T>>
    where
        T: StateSerializable + Identifiable;

    /// 查询实体（带上下文，如果不存在则返回错误）
    fn find_or_error_with_context(ctx: &Context, id: &str) -> Result<T>
    where
        T: StateSerializable + Identifiable;

    /// 保存实体
    fn save_with_context(&self, ctx: &mut Context) -> Result<()>
    where
        T: StateSerializable + Identifiable + Serialize;

    /// 删除实体
    fn delete_with_context(&self, ctx: &mut Context) -> Result<()>
    where
        T: Identifiable;

    /// 查询实体列表
    fn find_all() -> Result<Vec<T>>
    where
        T: StateSerializable + Identifiable;

    /// 查询实体列表（带上下文）
    fn find_all_with_context(ctx: &Context) -> Result<Vec<T>>
    where
        T: StateSerializable + Identifiable;

    /// 查询满足条件的实体列表
    fn find_where<F>(predicate: F) -> Result<Vec<T>>
    where
        F: Fn(&T) -> bool,
        T: StateSerializable + Identifiable;

    /// 查询满足条件的实体列表（带上下文）
    fn find_where_with_context<F>(ctx: &Context, predicate: F) -> Result<Vec<T>>
    where
        F: Fn(&T) -> bool,
        T: StateSerializable + Identifiable;
}

/// 批量操作特性
pub trait BatchOperations<T: Model + StateSerializable + Identifiable + Serialize> {
    /// 批量创建
    fn batch_create(entities: Vec<T>, ctx: &mut Context) -> Result<()> {
        for entity in entities {
            <T as StateSerializable>::save(&entity, ctx)?;
        }
        Ok(())
    }

    /// 批量删除
    fn batch_delete(entities: Vec<T>, ctx: &mut Context) -> Result<()> {
        for entity in entities {
            <T as StateSerializable>::delete(&entity, ctx)?;
        }
        Ok(())
    }
}

/// 状态上下文扩展特性
pub trait StateContextExt {
    /// 跟踪实体
    fn track_entity<T>(&mut self, entity: &T) -> Result<()>
    where
        T: Model + StateSerializable + Identifiable + Serialize;

    /// 删除实体
    fn delete_entity<T>(&mut self, entity_id: &str) -> Result<()>
    where
        T: Model;

    /// 获取状态差异
    fn get_diff(&self) -> StateDiff;
}

impl StateContextExt for Context {
    /// 跟踪实体
    fn track_entity<T>(&mut self, entity: &T) -> Result<()>
    where
        T: Model + StateSerializable + Identifiable + Serialize,
    {
        // 检查状态跟踪是否启用
        // 使用公共方法或属性来检查状态跟踪是否启用
        // 如果 Context 没有提供检查状态跟踪的公共方法，则始终跟踪

        // 获取实体ID
        let entity_id = <T as Identifiable>::get_id(entity);

        // 序列化实体
        let entity_data =
            serde_json::to_value(entity).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 记录实体变更
        self.entity_diffs.push(EntityDiff {
            entity_type: <T as Model>::get_entity_type().to_string(),
            id: entity_id,
            action: "update".to_string(),
            data: Some(entity_data),
        });

        Ok(())
    }

    /// 删除实体
    fn delete_entity<T>(&mut self, entity_id: &str) -> Result<()>
    where
        T: Model,
    {
        // 检查状态跟踪是否启用
        // 使用公共方法或属性来检查状态跟踪是否启用
        // 如果 Context 没有提供检查状态跟踪的公共方法，则始终跟踪

        // 记录实体删除
        self.entity_diffs.push(EntityDiff {
            entity_type: <T as Model>::get_entity_type().to_string(),
            id: entity_id.to_string(),
            action: "delete".to_string(),
            data: None,
        });

        Ok(())
    }

    /// 获取状态差异
    fn get_diff(&self) -> StateDiff {
        // 返回状态差异
        // 由于无法直接访问 Context 的私有字段 state，我们简化实现
        crate::state::StateDiff {
            changes: HashMap::new(), // 简化实现，不返回状态变更
            transaction_id: self.transaction_id.clone(),
            entities: HashMap::new(),
        }
    }
}

// 为Database实现QuerySupport特性
impl QuerySupport for Database {
    fn query_entity<T: StateSerializable + Identifiable + for<'de> Deserialize<'de>>(
        &self,
        id: &str,
    ) -> Result<Option<T>> {
        // 构建查询
        let entity_type = T::get_entity_type();
        let table_name = entity_type;
        let query = format!("SELECT * FROM {} WHERE id = ?", table_name);

        // 执行查询
        let rows = self.query(&query, &[id])?;

        if rows.is_empty() {
            return Ok(None);
        }

        // 反序列化第一行数据
        let row: &serde_json::Map<String, serde_json::Value> = &rows[0];
        let json_data = row
            .get("data")
            .ok_or_else(|| Error::SerializationError("Missing data column".to_string()))?;

        let json_str = json_data
            .as_str()
            .ok_or_else(|| Error::SerializationError("JSON data is not a string".to_string()))?;
        let entity: T = serde_json::from_str(json_str).map_err(|e| {
            Error::SerializationError(format!("Failed to deserialize entity: {}", e))
        })?;

        Ok(Some(entity))
    }

    fn query_entities<T: StateSerializable + Identifiable + for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        params: &[&str],
    ) -> Result<Vec<T>> {
        // 执行查询
        let rows: Vec<serde_json::Map<String, serde_json::Value>> = self.query(query, params)?;

        let mut entities = Vec::new();

        // 处理每一行数据
        for row in rows {
            let json_data = row
                .get("data")
                .ok_or_else(|| Error::SerializationError("Missing data column".to_string()))?;
            let json_str = json_data.as_str().ok_or_else(|| {
                Error::SerializationError("JSON data is not a string".to_string())
            })?;
            let entity: T = serde_json::from_str(json_str).map_err(|e| {
                Error::SerializationError(format!("Failed to deserialize entity: {}", e))
            })?;

            entities.push(entity);
        }

        Ok(entities)
    }
}

/// 模型工具特性 - 为模型提供快捷方法
pub trait ModelUtils: Model + Sized {
    /// 从数据库中获取实体
    fn find(db: &Database, id: &str) -> Result<Option<Self>>
    where
        Self: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    {
        db.query_entity(id)
    }

    /// 从数据库中获取实体，如果不存在则返回错误
    fn find_or_error(db: &Database, id: &str) -> Result<Self>
    where
        Self: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    {
        db.query_entity(id)
            .entity_not_found(<Self as Identifiable>::get_entity_type(), id)
    }

    /// 从上下文中获取实体
    fn find_with_context(ctx: &mut Context, id: &str) -> Result<Option<Self>>
    where
        Self: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    {
        ctx.db().query_entity_with_context(ctx, id)
    }

    /// 从上下文中获取实体，如果不存在则返回错误
    fn find_or_error_with_context(ctx: &mut Context, id: &str) -> Result<Self>
    where
        Self: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    {
        ctx.db()
            .query_entity_with_context(ctx, id)
            .entity_not_found(<Self as Identifiable>::get_entity_type(), id)
    }

    /// 保存实体
    fn save(&self, ctx: &mut Context) -> Result<()>
    where
        Self: StateSerializable + Identifiable + Serialize,
    {
        let db = ctx.db().clone();
        db.save_entity(ctx, self)
    }

    /// 删除实体
    fn delete(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Identifiable,
    {
        let db = ctx.db().clone();
        db.delete_entity::<Self>(ctx, &<Self as Identifiable>::get_id(self))
    }

    /// 创建实体
    fn create(ctx: &mut Context, entity: &Self) -> Result<()>
    where
        Self: StateSerializable + Identifiable + Serialize,
    {
        let db = ctx.db().clone();
        db.save_entity(ctx, entity)
    }

    /// 查询实体
    fn query(ctx: &mut Context, query: &str, params: &[&str]) -> Result<Vec<Self>>
    where
        Self: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    {
        ctx.db()
            .query_entities_with_context::<Self>(ctx, query, params)
    }
}

/// 一对多关系定义
pub struct HasMany<T> {
    _phantom: std::marker::PhantomData<T>,
    foreign_key: String,
    parent_id: String,
}

impl<T: crate::model::Model + for<'de> Deserialize<'de> + Identifiable + StateSerializable>
    HasMany<T>
{
    /// 创建新的一对多关系
    pub fn new(parent_id: &str, foreign_key: &str) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            foreign_key: foreign_key.to_string(),
            parent_id: parent_id.to_string(),
        }
    }

    /// 获取关联的实体列表
    pub fn get(&self, ctx: &mut Context) -> Result<Vec<T>> {
        let table_name = <T as Identifiable>::get_entity_type();
        let query = format!(
            "SELECT * FROM {} WHERE {} = ?",
            table_name, self.foreign_key
        );

        T::query(ctx, &query, &[&self.parent_id])
    }

    /// 添加关联实体
    pub fn add(&self, ctx: &mut Context, entity: &mut T) -> Result<()>
    where
        T: SetField,
    {
        // 设置外键
        entity.set_field(&self.foreign_key, &self.parent_id)?;

        // 保存实体
        <T as ModelExt>::save(entity, ctx)
    }

    /// 清除关联
    pub fn clear(&self, ctx: &mut Context) -> Result<()> {
        let table_name = <T as Identifiable>::get_entity_type();
        let query = format!("DELETE FROM {} WHERE {} = ?", table_name, self.foreign_key);

        ctx.db()
            .execute_with_context(&query, &[&self.parent_id], ctx)
    }
}

/// 字段设置特性
pub trait SetField {
    /// 设置字段值
    fn set_field(&mut self, field: &str, value: &str) -> Result<()>;
}

/// 模型特性扩展 - 为模型提供状态序列化相关的方法
pub trait ModelExt: Serialize + for<'de> Deserialize<'de> + Clone {
    /// 保存实体
    fn save(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Serialize + StateSerializable + Identifiable + Model,
    {
        ctx.track_entity(self)
    }

    /// 删除实体
    fn delete(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Identifiable + Model,
    {
        ctx.delete_entity::<Self>(&<Self as Identifiable>::get_id(self))
    }

    /// 获取状态差异
    fn get_diff(&self) -> Result<serde_json::Value>
    where
        Self: Serialize,
    {
        serde_json::to_value(self).map_err(|e| Error::SerializationError(e.to_string()))
    }
}

// 为所有实现了 Model 的类型自动实现 ModelExt
impl<T: crate::model::Model> ModelExt for T {}
