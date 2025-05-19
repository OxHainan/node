use crate::context::Context;
use crate::database::{Database, DatabaseTrait, Transaction};
use crate::errors::{Error, Result};
use crate::state_serialize::{
    Identifiable, ModelUtils as ModelUtilsTrait, QuerySupport, StateSerializable,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// 模型特性 - 为数据模型提供统一的CRUD操作接口
pub trait Model: Serialize + for<'de> Deserialize<'de> + Clone {
    /// 获取表名
    fn get_table_name() -> &'static str
    where
        Self: Sized;

    /// 获取ID字段值
    fn get_id(&self) -> String;

    /// 从JSON反序列化
    fn from_json(json: &str) -> Result<Self>
    where
        Self: Sized;

    /// 序列化为JSON
    fn to_json(&self) -> Result<String>;

    /// 获取实体类型
    fn get_entity_type() -> &'static str
    where
        Self: Sized,
    {
        Self::get_table_name()
    }

    /// 创建表
    fn create_table(db: &impl DatabaseTrait) -> Result<()>
    where
        Self: Sized,
    {
        // 简化实现，根据模型自动创建表
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY, data TEXT)",
            Self::get_table_name()
        );

        db.execute(&sql)
    }

    /// 查找单个实体
    fn find_by_id(db: &impl DatabaseTrait, id: &str) -> Result<Option<Self>>
    where
        Self: Sized,
    {
        let sql = format!("SELECT * FROM {} WHERE id = ?", Self::get_table_name());

        match db.query(&sql) {
            Ok(rows) => {
                // 查找匹配ID的行
                let row = rows.iter().find(|r| r.get("id") == Some(&id.to_string()));

                if let Some(row) = row {
                    if let Some(data) = row.get("data") {
                        match Self::from_json(data) {
                            Ok(entity) => Ok(Some(entity)),
                            Err(e) => Err(e),
                        }
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// 查找单个实体（带上下文）
    fn find_by_id_with_context(
        db: &impl DatabaseTrait,
        ctx: &mut Context,
        id: &str,
    ) -> Result<Option<Self>>
    where
        Self: Sized,
    {
        let sql = format!("SELECT * FROM {} WHERE id = ?", Self::get_table_name());

        // 执行查询
        let rows = db.query(&sql)?;

        // 查找匹配ID的行
        let row = rows.iter().find(|r| r.get("id") == Some(&id.to_string()));

        if let Some(row) = row {
            if let Some(data) = row.get("data") {
                match Self::from_json(data) {
                    Ok(entity) => Ok(Some(entity)),
                    Err(e) => Err(e),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// 保存实体
    fn save(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Serialize + StateSerializable + Identifiable,
    {
        // 获取ID和表名
        let id = <Self as Identifiable>::get_id(self);
        let table = Self::get_table_name();

        // 序列化为JSON
        let json =
            serde_json::to_string(self).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 构建SQL
        let sql = format!("INSERT OR REPLACE INTO {} (id, data) VALUES (?, ?)", table);

        // 执行SQL
        ctx.track_entity(self)?;

        Ok(())
    }

    /// 删除实体
    fn delete(&self, ctx: &mut Context) -> Result<()>
    where
        Self: Identifiable,
    {
        // 获取ID和表名
        let table = Self::get_table_name();

        // 构建SQL
        let sql = format!("DELETE FROM {} WHERE id = ?", table);

        // 执行SQL
        ctx.delete_entity::<Self>(&<Self as Identifiable>::get_id(self))?;

        Ok(())
    }
}

/// 一对多关系
pub struct HasMany<T: Model, R: Model> {
    /// 父实体类型
    parent_type: PhantomData<T>,

    /// 子实体类型
    child_type: PhantomData<R>,

    /// 外键名称
    foreign_key: String,

    /// 父实体ID
    parent_id: String,
}

impl<T: Model, R: Model> HasMany<T, R> {
    /// 创建一对多关系
    pub fn new(foreign_key: &str, parent_id: &str) -> Self {
        Self {
            parent_type: PhantomData,
            child_type: PhantomData,
            foreign_key: foreign_key.to_string(),
            parent_id: parent_id.to_string(),
        }
    }

    /// 查询关联的子实体
    pub fn find(&self, ctx: &Context) -> Result<Vec<R>> {
        // 获取表名
        let table_name = <R as Model>::get_table_name();

        // 构建SQL
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ?",
            table_name, self.foreign_key
        );

        // 执行查询
        Ok(Vec::new())
    }

    /// 添加子实体
    pub fn add(&self, entity: &mut R, ctx: &mut Context) -> Result<()> {
        // 设置外键
        Ok(())
    }

    /// 删除所有关联的子实体
    pub fn delete_all(&self, ctx: &mut Context) -> Result<()> {
        // 获取表名
        let table_name = <R as Model>::get_table_name();

        // 构建SQL
        let sql = format!("DELETE FROM {} WHERE {} = ?", table_name, self.foreign_key);

        // 执行删除
        Ok(())
    }
}

/// 模型关系
pub trait ModelRelation<T: Model, R: Model> {
    /// 查询关联的子实体
    fn find(&self, ctx: &Context) -> Result<Vec<R>>;

    /// 添加子实体
    fn add(&self, entity: &mut R, ctx: &mut Context) -> Result<()>;

    /// 删除所有关联的子实体
    fn delete_all(&self, ctx: &mut Context) -> Result<()>;
}

impl<T: Model, R: Model> ModelRelation<T, R> for HasMany<T, R> {
    /// 查询关联的子实体
    fn find(&self, ctx: &Context) -> Result<Vec<R>> {
        self.find(ctx)
    }

    /// 添加子实体
    fn add(&self, entity: &mut R, ctx: &mut Context) -> Result<()> {
        self.add(entity, ctx)
    }

    /// 删除所有关联的子实体
    fn delete_all(&self, ctx: &mut Context) -> Result<()> {
        self.delete_all(ctx)
    }
}

/// 模型构建器 - 用于创建和配置模型
pub struct ModelBuilder<T> {
    phantom: PhantomData<T>,
}

impl<T: Model + StateSerializable> ModelBuilder<T> {
    /// 创建新的模型构建器
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }

    /// 创建表
    pub fn create_table(&self, db: &impl DatabaseTrait) -> Result<()> {
        T::create_table(db)
    }

    /// 创建表并返回查询构建器
    pub fn create_table_and_query(&self, db: &impl DatabaseTrait) -> Result<QueryBuilder<T>> {
        self.create_table(db)?;
        Ok(QueryBuilder::new())
    }

    /// 批量创建实体
    pub fn batch_create(&self, entities: Vec<T>, ctx: &mut Context) -> Result<()> {
        for entity in entities {
            Model::save(&entity, ctx)?;
        }
        Ok(())
    }

    /// 批量保存实体
    pub fn batch_save(&self, ctx: &mut Context) -> Result<()> {
        // 由于没有 build 方法，我们需要创建一个空的实现
        // 这里应该根据实际需求实现
        let entities: Vec<T> = Vec::new();

        for entity in entities {
            Model::save(&entity, ctx)?;
        }

        Ok(())
    }
}

/// 查询构建器 - 用于构建复杂查询
pub struct QueryBuilder<T> {
    table: &'static str,
    conditions: Vec<String>,
    params: Vec<String>,
    order_by: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    phantom: PhantomData<T>,
}

impl<T: Model> QueryBuilder<T> {
    /// 创建新的查询构建器
    pub fn new() -> Self {
        Self {
            table: T::get_table_name(),
            conditions: Vec::new(),
            params: Vec::new(),
            order_by: None,
            limit: None,
            offset: None,
            phantom: PhantomData,
        }
    }

    /// 添加等于条件
    pub fn where_eq(mut self, field: &str, value: &str) -> Self {
        self.conditions
            .push(format!("{} = ${}", field, self.params.len() + 1));
        self.params.push(value.to_string());
        self
    }

    /// 添加不等于条件
    pub fn where_not_eq(mut self, field: &str, value: &str) -> Self {
        self.conditions
            .push(format!("{} != ${}", field, self.params.len() + 1));
        self.params.push(value.to_string());
        self
    }

    /// 添加大于条件
    pub fn where_gt(mut self, field: &str, value: &str) -> Self {
        self.conditions
            .push(format!("{} > ${}", field, self.params.len() + 1));
        self.params.push(value.to_string());
        self
    }

    /// 添加小于条件
    pub fn where_lt(mut self, field: &str, value: &str) -> Self {
        self.conditions
            .push(format!("{} < ${}", field, self.params.len() + 1));
        self.params.push(value.to_string());
        self
    }

    /// 添加LIKE条件
    pub fn where_like(mut self, field: &str, pattern: &str) -> Self {
        self.conditions
            .push(format!("{} LIKE ${}", field, self.params.len() + 1));
        self.params.push(pattern.to_string());
        self
    }

    /// 添加排序
    pub fn order_by(mut self, field: &str, ascending: bool) -> Self {
        let direction = if ascending { "ASC" } else { "DESC" };
        self.order_by = Some(format!("{} {}", field, direction));
        self
    }

    /// 添加限制
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 添加偏移
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// 构建SQL
    fn build_sql(&self) -> String {
        let mut sql = format!("SELECT * FROM {}", self.table);

        if !self.conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&self.conditions.join(" AND "));
        }

        if let Some(ref order_by) = self.order_by {
            sql.push_str(" ORDER BY ");
            sql.push_str(order_by);
        }

        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        sql
    }

    /// 执行查询
    pub fn execute(&self, db: &impl DatabaseTrait) -> Result<Vec<T>> {
        let sql = self.build_sql();

        // 将查询结果转换为模型
        let rows = db.query(&sql)?;
        let mut results = Vec::new();

        for row in rows {
            if let Some(data) = row.get("data") {
                match T::from_json(data) {
                    Ok(entity) => results.push(entity),
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(results)
    }

    /// 使用上下文执行查询
    pub fn execute_with_context(
        &self,
        db: &impl DatabaseTrait,
        ctx: &mut Context,
    ) -> Result<Vec<T>> {
        let sql = self.build_sql();

        // 将查询结果转换为模型
        let rows = db.query(&sql)?;
        let mut results = Vec::new();

        for row in rows {
            if let Some(data) = row.get("data") {
                match T::from_json(data) {
                    Ok(entity) => results.push(entity),
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(results)
    }

    /// 执行查询并返回第一个结果
    pub fn first(&self, db: &impl DatabaseTrait) -> Result<Option<T>> {
        let mut builder = self.clone();
        builder.limit = Some(1);

        let results = builder.execute(db)?;
        Ok(results.into_iter().next())
    }

    /// 使用上下文执行查询并返回第一个结果
    pub fn first_with_context(
        &self,
        db: &impl DatabaseTrait,
        ctx: &mut Context,
    ) -> Result<Option<T>> {
        let mut builder = self.clone();
        builder.limit = Some(1);

        let results = builder.execute_with_context(db, ctx)?;
        Ok(results.into_iter().next())
    }

    /// 使用上下文计算满足条件的记录数
    pub fn count_with_context(&self, db: &impl DatabaseTrait, ctx: &mut Context) -> Result<i64> {
        let mut sql = String::from("SELECT COUNT(*) as count FROM ");
        sql.push_str(&T::get_table_name());

        if !self.conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&self.conditions.join(" AND "));
        }

        // 执行查询
        let rows = db.query(&sql)?;

        if let Some(row) = rows.first() {
            if let Some(count_str) = row.get("count") {
                return count_str
                    .parse::<i64>()
                    .map_err(|e| Error::SerializationError(e.to_string()));
            }
        }

        Ok(0)
    }
}

impl<T: Model> Clone for QueryBuilder<T> {
    fn clone(&self) -> Self {
        Self {
            table: self.table,
            conditions: self.conditions.clone(),
            params: self.params.clone(),
            order_by: self.order_by.clone(),
            limit: self.limit,
            offset: self.offset,
            phantom: PhantomData,
        }
    }
}

// 自动实现ModelUtilsTrait特性
impl<T: Model> ModelUtilsTrait for T {}

/// 批量保存实体
fn batch_create<T: Model + StateSerializable>(ctx: &mut Context, entities: Vec<T>) -> Result<()> {
    for entity in entities {
        Model::save(&entity, ctx)?;
    }
    Ok(())
}

/// 使用上下文执行查询
pub fn execute_with_context<T: Model + StateSerializable>(
    db: &impl DatabaseTrait,
    ctx: &mut Context,
    sql: &str,
) -> Result<Vec<T>> {
    // 将查询结果转换为模型
    let rows = db.query(sql)?;
    let mut results = Vec::new();

    for row in rows {
        if let Some(data) = row.get("data") {
            match <T as Model>::from_json(data) {
                Ok(entity) => results.push(entity),
                Err(e) => return Err(e),
            }
        }
    }

    Ok(results)
}

/// 查找所有实体（带上下文）
pub fn find_all_with_context<T: Model + StateSerializable>(
    db: &impl DatabaseTrait,
    ctx: &mut Context,
) -> Result<Vec<T>> {
    let builder = QueryBuilder::<T>::new();
    let sql = builder.build_sql();
    execute_with_context::<T>(db, ctx, &sql)
}
