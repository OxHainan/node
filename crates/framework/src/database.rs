use crate::context::Context;
use crate::errors::{Error, Result};
use crate::model::Model;
use crate::state::StateDiff;
use crate::state_serialize::{Identifiable, StateSerializable};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 数据库特性 - 定义数据库通用接口
pub trait DatabaseTrait {
    /// 执行SQL查询
    fn execute(&self, sql: &str) -> Result<()>;

    /// 查询数据
    fn query(&self, sql: &str) -> Result<Vec<HashMap<String, String>>>;

    /// 获取表数据
    fn get_table(&self, table: &str) -> Result<Option<Vec<HashMap<String, String>>>>;

    /// 获取数据
    fn get(&self, table: &str, id: &str) -> Result<Option<String>>;

    /// 插入数据
    fn insert(&self, table: &str, id: &str, data: &str) -> Result<()>;

    /// 更新数据
    fn update(&self, table: &str, id: &str, data: &str) -> Result<()>;

    /// 删除数据
    fn delete(&self, table: &str, id: &str) -> Result<()>;
}

/// 事务特性 - 定义事务操作
pub trait Transaction {
    /// 开始事务
    fn begin(&self) -> Result<()>;

    /// 提交事务
    fn commit(&self) -> Result<()>;

    /// 回滚事务
    fn rollback(&self) -> Result<()>;
}

/// 内存数据库 - 简单的内存数据库实现
#[derive(Debug, Clone)]
pub struct Database {
    /// 表数据
    tables: Arc<Mutex<HashMap<String, Vec<HashMap<String, String>>>>>,

    /// 查询日志
    query_log: Arc<Mutex<Vec<String>>>,

    /// 上下文
    ctx: Arc<Mutex<Context>>,
}

// 内存数据库别名
pub type InMemoryDatabase = Database;

impl Database {
    /// 创建内存数据库
    pub fn in_memory() -> Self {
        Self {
            tables: Arc::new(Mutex::new(HashMap::new())),
            query_log: Arc::new(Mutex::new(Vec::new())),
            ctx: Arc::new(Mutex::new(Context::new())),
        }
    }

    /// 创建内存数据库（别名）
    pub fn new_in_memory() -> Result<Self> {
        Ok(Self::in_memory())
    }

    /// 创建新的数据库（内存数据库）
    pub fn new() -> Self {
        Self::in_memory()
    }

    /// 从环境变量创建数据库（兼容API，实际仍使用内存数据库）
    pub fn from_env() -> Self {
        Self::in_memory()
    }

    /// 执行SQL语句
    pub fn execute(&self, sql: &str) -> Result<()> {
        // 记录查询
        self.query_log.lock().unwrap().push(sql.to_string());

        // 解析SQL
        let sql = sql.trim();

        if sql.starts_with("CREATE TABLE") {
            // 创建表
            let table_name = extract_table_name(sql).ok_or(Error::DatabaseError(
                "Failed to extract table name".to_string(),
            ))?;

            if let Ok(mut tables) = self.tables.lock() {
                if !tables.contains_key(&table_name) {
                    tables.insert(table_name.clone(), Vec::new());
                }
            }
        } else if sql.starts_with("INSERT") {
            // 插入数据
            let (table_name, id, data) = extract_insert_data(sql).ok_or(Error::DatabaseError(
                "Failed to extract insert data".to_string(),
            ))?;

            if let Ok(mut tables) = self.tables.lock() {
                if let Some(table) = tables.get_mut(&table_name) {
                    // 检查是否已存在
                    let existing = table.iter().position(|row| row.get("id") == Some(&id));

                    if let Some(index) = existing {
                        // 更新现有数据
                        table[index].insert("data".to_string(), data.to_string());
                    } else {
                        // 插入新数据
                        let mut row = HashMap::new();
                        row.insert("id".to_string(), id.to_string());
                        row.insert("data".to_string(), data.to_string());
                        table.push(row);
                    }
                } else {
                    return Err(Error::DatabaseError(format!(
                        "Table {} not found",
                        table_name
                    )));
                }
            }
        } else if sql.starts_with("UPDATE") {
            // 更新数据
            let (table_name, id, data) = extract_update_data(sql).ok_or(Error::DatabaseError(
                "Failed to extract update data".to_string(),
            ))?;

            if let Ok(mut tables) = self.tables.lock() {
                if let Some(table) = tables.get_mut(&table_name) {
                    // 查找数据
                    let existing = table.iter().position(|row| row.get("id") == Some(&id));

                    if let Some(index) = existing {
                        // 更新数据
                        table[index].insert("data".to_string(), data.to_string());
                    } else {
                        return Err(Error::DatabaseError(format!(
                            "Record with id {} not found in table {}",
                            id, table_name
                        )));
                    }
                } else {
                    return Err(Error::DatabaseError(format!(
                        "Table {} not found",
                        table_name
                    )));
                }
            }
        } else if sql.starts_with("DELETE") {
            // 删除数据
            let (table_name, id) = extract_delete_data(sql).ok_or(Error::DatabaseError(
                "Failed to extract delete data".to_string(),
            ))?;

            if let Ok(mut tables) = self.tables.lock() {
                if let Some(table) = tables.get_mut(&table_name) {
                    // 删除数据
                    table.retain(|row| row.get("id") != Some(&id));
                } else {
                    return Err(Error::DatabaseError(format!(
                        "Table {} not found",
                        table_name
                    )));
                }
            }
        } else if sql.starts_with("DROP TABLE") {
            // 删除表
            let table_name = extract_drop_table_name(sql).ok_or(Error::DatabaseError(
                "Failed to extract drop table name".to_string(),
            ))?;

            if let Ok(mut tables) = self.tables.lock() {
                tables.remove(&table_name);
            }
        }

        Ok(())
    }

    /// 使用上下文执行查询
    pub fn execute_with_context(
        &self,
        sql: &str,
        params: &[&str],
        ctx: &mut Context,
    ) -> Result<()> {
        // 记录查询
        self.query_log.lock().unwrap().push(format!(
            "EXECUTE WITH CONTEXT: {} with params: {:?}",
            sql, params
        ));

        // 执行SQL
        self.execute(sql)?;

        // 记录操作
        let query_id = uuid::Uuid::new_v4().to_string();
        let query_key = format!("execute:{}", query_id);

        ctx.set_state_key(&query_key, "executed");

        Ok(())
    }

    /// 执行查询并返回单个结果
    pub fn query_one<T: for<'de> Deserialize<'de> + Default>(
        &self,
        sql: &str,
        params: &[&str],
    ) -> Result<Option<T>> {
        // 记录查询
        self.query_log
            .lock()
            .unwrap()
            .push(format!("QUERY: {} with params: {:?}", sql, params));

        // 模拟查询结果
        if sql.contains("SELECT") {
            // 返回默认值作为模拟结果
            return Ok(Some(T::default()));
        }

        Ok(None)
    }

    /// 使用上下文执行查询并返回单个结果
    pub fn query_one_with_context<T: for<'de> Deserialize<'de> + Default>(
        &self,
        sql: &str,
        params: &[&str],
        ctx: &mut Context,
    ) -> Result<Option<T>> {
        // 记录查询
        self.query_log.lock().unwrap().push(format!(
            "QUERY ONE WITH CONTEXT: {} with params: {:?}",
            sql, params
        ));

        // 记录查询操作（只读操作不会影响状态，但可能需要跟踪以支持通知）
        if !sql.trim().to_uppercase().starts_with("SELECT") {
            return Err(Error::DatabaseError(
                "Only SELECT operations are allowed with query_one_with_context".to_string(),
            ));
        }

        // 记录查询操作，生成唯一ID用于通知系统
        let query_id = uuid::Uuid::new_v4().to_string();
        let query_key = format!("query:{}", query_id);

        // 在实际实现中，这里应该将查询结果序列化并存储
        // 简化起见，只记录查询发生的事实
        ctx.set_state_key(&query_key, "query_executed");

        // 模拟查询结果
        Ok(Some(T::default()))
    }

    /// 执行查询并返回多个结果
    pub fn query<T: for<'de> Deserialize<'de>>(
        &self,
        sql: &str,
        params: &[&str],
    ) -> Result<Vec<T>> {
        // 记录查询
        self.query_log
            .lock()
            .unwrap()
            .push(format!("QUERY: {} with params: {:?}", sql, params));

        // 返回空结果集
        Ok(Vec::new())
    }

    /// 使用上下文执行查询
    pub fn query_with_context<T: for<'de> Deserialize<'de>>(
        &self,
        sql: &str,
        params: &[&str],
        ctx: &mut Context,
    ) -> Result<Vec<T>> {
        // 记录查询
        self.query_log.lock().unwrap().push(format!(
            "QUERY WITH CONTEXT: {} with params: {:?}",
            sql, params
        ));

        // 记录查询操作（只读操作不会影响状态，但可能需要跟踪以支持通知）
        if !sql.trim().to_uppercase().starts_with("SELECT") {
            return Err(Error::DatabaseError(
                "Only SELECT operations are allowed with query_with_context".to_string(),
            ));
        }

        // 记录查询操作，生成唯一ID用于通知系统
        let query_id = uuid::Uuid::new_v4().to_string();
        let query_key = format!("query:{}", query_id);

        // 在实际实现中，这里应该将查询结果序列化并存储
        // 简化起见，只记录查询发生的事实
        ctx.set_state_key(&query_key, "query_executed");

        // 模拟查询结果
        Ok(Vec::new())
    }

    /// 使用事务执行操作
    pub fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&dyn Transaction) -> Result<R>,
    {
        // 生成事务ID
        let tx_id = uuid::Uuid::new_v4().to_string();

        // 创建事务
        let tx = TransactionImpl::new(self, tx_id);

        // 开始事务
        tx.begin()?;

        // 执行操作
        match f(&tx) {
            Ok(result) => {
                // 提交事务
                tx.commit()?;
                Ok(result)
            }
            Err(e) => {
                // 回滚事务
                tx.rollback()?;
                Err(e)
            }
        }
    }

    /// 使用上下文执行事务
    pub fn transaction_with_context<F, R>(
        &self,
        ctx: &mut Context,
        f: F,
    ) -> Result<(R, crate::context::StateDiff)>
    where
        F: FnOnce(&dyn Transaction, &mut Context) -> Result<R>,
    {
        // 生成事务ID
        let tx_id = uuid::Uuid::new_v4().to_string();

        // 创建事务
        let tx = TransactionImpl::new(self, tx_id.clone());

        // 设置事务ID
        ctx.set_transaction_id(&tx_id);

        // 开始事务
        tx.begin()?;

        // 执行操作
        match f(&tx, ctx) {
            Ok(result) => {
                // 提交事务
                tx.commit()?;

                // 获取状态差异
                let diff = ctx.get_diff();

                Ok((result, diff))
            }
            Err(e) => {
                // 回滚事务
                tx.rollback()?;

                // 取消事务
                ctx.cancel_transaction();

                Err(e)
            }
        }
    }

    /// 获取查询日志
    pub fn get_query_log(&self) -> Vec<String> {
        self.query_log.lock().unwrap().clone()
    }

    /// 获取表中的数据
    pub fn get(&self, table: &str, id: &str) -> Result<Option<String>> {
        let tables = self.tables.lock().unwrap();
        if let Some(rows) = tables.get(table) {
            for row in rows {
                if let Some(row_id) = row.get("id") {
                    if row_id == id {
                        return Ok(row.get("data").cloned());
                    }
                }
            }
        }
        Ok(None)
    }

    /// 插入数据到表中
    pub fn insert(&self, table: &str, id: &str, data: &str) -> Result<()> {
        let mut tables = self.tables.lock().unwrap();
        let rows = tables.entry(table.to_string()).or_insert_with(Vec::new);

        // 检查是否已存在
        let mut found = false;
        for row in rows.iter_mut() {
            if let Some(row_id) = row.get("id") {
                if row_id == id {
                    // 更新现有行
                    row.insert("data".to_string(), data.to_string());
                    found = true;
                    break;
                }
            }
        }

        // 如果不存在，添加新行
        if !found {
            let mut new_row = HashMap::new();
            new_row.insert("id".to_string(), id.to_string());
            new_row.insert("data".to_string(), data.to_string());
            rows.push(new_row);
        }

        Ok(())
    }

    /// 从表中删除数据
    pub fn delete(&self, table: &str, id: &str) -> Result<()> {
        let mut tables = self.tables.lock().unwrap();
        if let Some(rows) = tables.get_mut(table) {
            rows.retain(|row| {
                if let Some(row_id) = row.get("id") {
                    row_id != id
                } else {
                    true
                }
            });
        }
        Ok(())
    }

    /// 获取整个表
    pub fn get_table(&self, table: &str) -> Result<Option<Vec<HashMap<String, String>>>> {
        let tables = self.tables.lock().unwrap();
        if let Some(rows) = tables.get(table) {
            Ok(Some(rows.clone()))
        } else {
            Ok(None)
        }
    }

    /// 使用上下文查询单个实体
    pub fn query_entity_with_context<
        T: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    >(
        &self,
        ctx: &mut Context,
        id: &str,
    ) -> Result<Option<T>> {
        // 构建查询
        let table_name = <T as Identifiable>::get_entity_type();
        let sql = format!("SELECT * FROM {} WHERE id = ?", table_name);

        // 执行查询
        let results = self.query_with_context::<serde_json::Map<String, serde_json::Value>>(
            &sql,
            &[id],
            ctx,
        )?;

        if let Some(row) = results.into_iter().next() {
            if let Some(data) = row.get("data") {
                if let Some(data_str) = data.as_str() {
                    // 反序列化实体
                    match serde_json::from_str::<T>(data_str) {
                        Ok(entity) => return Ok(Some(entity)),
                        Err(e) => return Err(Error::DeserializationError(e.to_string())),
                    }
                }
            }
        }

        Ok(None)
    }

    /// 使用上下文查询多个实体
    pub fn query_entities_with_context<
        T: StateSerializable + Identifiable + for<'de> Deserialize<'de>,
    >(
        &self,
        ctx: &mut Context,
        query: &str,
        params: &[&str],
    ) -> Result<Vec<T>> {
        // 执行查询
        let results = self
            .query_with_context::<serde_json::Map<String, serde_json::Value>>(query, params, ctx)?;

        let mut entities = Vec::new();

        // 处理每一行数据
        for row in results {
            if let Some(data) = row.get("data") {
                if let Some(data_str) = data.as_str() {
                    // 反序列化实体
                    match serde_json::from_str::<T>(data_str) {
                        Ok(entity) => entities.push(entity),
                        Err(e) => return Err(Error::DeserializationError(e.to_string())),
                    }
                }
            }
        }

        Ok(entities)
    }

    /// 保存实体
    pub fn save_entity<T: StateSerializable + Identifiable + Serialize + crate::model::Model>(
        &self,
        ctx: &mut Context,
        entity: &T,
    ) -> Result<()> {
        // 获取ID和表名
        let id = <T as Identifiable>::get_id(entity);
        let table_name = <T as Identifiable>::get_entity_type();

        // 序列化实体
        let data =
            serde_json::to_string(entity).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 构建SQL
        let sql = format!(
            "INSERT OR REPLACE INTO {} (id, data) VALUES (?, ?)",
            table_name
        );

        // 执行SQL
        self.execute_with_context(&sql, &[&id, &data], ctx)?;

        // 跟踪实体变更
        ctx.track_entity(entity)?;

        Ok(())
    }

    /// 批量保存实体
    pub fn batch_save_entities<
        T: StateSerializable + Identifiable + Serialize + crate::model::Model,
    >(
        &self,
        ctx: &mut Context,
        entities: &[T],
    ) -> Result<()> {
        for entity in entities {
            self.save_entity(ctx, entity)?;
        }

        Ok(())
    }

    /// 删除实体
    pub fn delete_entity<T: Identifiable + crate::model::Model>(
        &self,
        ctx: &mut Context,
        id: &str,
    ) -> Result<()> {
        // 获取表名
        let table_name = <T as Identifiable>::get_entity_type();

        // 构建SQL
        let sql = format!("DELETE FROM {} WHERE id = ?", table_name);

        // 执行SQL
        self.execute_with_context(&sql, &[id], ctx)?;

        // 记录实体删除
        ctx.delete_entity::<T>(id)?;

        Ok(())
    }
}

/// 辅助函数：从CREATE TABLE语句中提取表名
fn extract_table_name(sql: &str) -> Option<String> {
    let sql = sql.trim().to_uppercase();

    if sql.starts_with("CREATE TABLE") {
        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(
                parts[2]
                    .trim_matches(|c| c == '(' || c == ')' || c == ';')
                    .to_string(),
            );
        }
    } else if sql.starts_with("INSERT INTO") {
        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(
                parts[2]
                    .trim_matches(|c| c == '(' || c == ')' || c == ';')
                    .to_string(),
            );
        }
    } else if sql.starts_with("UPDATE") {
        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.len() >= 2 {
            return Some(
                parts[1]
                    .trim_matches(|c| c == '(' || c == ')' || c == ';')
                    .to_string(),
            );
        }
    } else if sql.starts_with("DELETE FROM") {
        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(
                parts[2]
                    .trim_matches(|c| c == '(' || c == ')' || c == ';')
                    .to_string(),
            );
        }
    } else if sql.starts_with("SELECT") {
        // 查找FROM后面的表名
        if let Some(from_pos) = sql.find("FROM") {
            let after_from = &sql[from_pos + 4..];
            let parts: Vec<&str> = after_from.split_whitespace().collect();
            if !parts.is_empty() {
                return Some(
                    parts[0]
                        .trim_matches(|c| c == '(' || c == ')' || c == ';')
                        .to_string(),
                );
            }
        }
    } else if sql.starts_with("DROP TABLE") {
        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(
                parts[2]
                    .trim_matches(|c| c == '(' || c == ')' || c == ';')
                    .to_string(),
            );
        }
    }

    None
}

fn extract_insert_data(sql: &str) -> Option<(String, String, String)> {
    let sql = sql.trim();

    if sql.to_uppercase().starts_with("INSERT INTO") {
        // 解析表名
        let table_name = extract_table_name(sql)?;

        // 解析ID和数据
        // 简化实现，假设SQL格式为: INSERT INTO table (id, data) VALUES ('id_value', 'data_value')
        if let Some(values_pos) = sql.to_uppercase().find("VALUES") {
            let values_part = &sql[values_pos + 6..];
            let values_part = values_part
                .trim()
                .trim_start_matches('(')
                .trim_end_matches(|c| c == ')' || c == ';');

            let values: Vec<&str> = values_part.split(',').collect();
            if values.len() >= 2 {
                let id = values[0]
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();
                let data = values[1]
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();

                return Some((table_name, id, data));
            }
        }
    }

    None
}

fn extract_update_data(sql: &str) -> Option<(String, String, String)> {
    let sql = sql.trim();

    if sql.to_uppercase().starts_with("UPDATE") {
        // 解析表名
        let table_name = extract_table_name(sql)?;

        // 解析ID和数据
        // 简化实现，假设SQL格式为: UPDATE table SET data = 'data_value' WHERE id = 'id_value'
        if let Some(where_pos) = sql.to_uppercase().find("WHERE") {
            let set_part = &sql[..where_pos];
            let where_part = &sql[where_pos..];

            // 解析SET部分
            if let Some(set_pos) = set_part.to_uppercase().find("SET") {
                let set_data = &set_part[set_pos + 3..].trim();

                // 解析data值
                if let Some(eq_pos) = set_data.find('=') {
                    let field = set_data[..eq_pos].trim();
                    if field == "data" {
                        let data = set_data[eq_pos + 1..]
                            .trim()
                            .trim_matches('\'')
                            .trim_matches('"')
                            .to_string();

                        // 解析WHERE部分
                        if let Some(id_pos) = where_part.to_uppercase().find("ID =") {
                            let id_part = &where_part[id_pos + 4..].trim();
                            let id = id_part
                                .trim_matches('\'')
                                .trim_matches('"')
                                .trim_end_matches(';')
                                .to_string();

                            return Some((table_name, id, data));
                        }
                    }
                }
            }
        }
    }

    None
}

fn extract_delete_data(sql: &str) -> Option<(String, String)> {
    let sql = sql.trim();

    if sql.to_uppercase().starts_with("DELETE FROM") {
        // 解析表名
        let table_name = extract_table_name(sql)?;

        // 解析ID
        // 简化实现，假设SQL格式为: DELETE FROM table WHERE id = 'id_value'
        if let Some(where_pos) = sql.to_uppercase().find("WHERE") {
            let where_part = &sql[where_pos..];

            // 解析WHERE部分
            if let Some(id_pos) = where_part.to_uppercase().find("ID =") {
                let id_part = &where_part[id_pos + 4..].trim();
                let id = id_part
                    .trim_matches('\'')
                    .trim_matches('"')
                    .trim_end_matches(';')
                    .to_string();

                return Some((table_name, id));
            }
        }
    }

    None
}

fn extract_drop_table_name(sql: &str) -> Option<String> {
    extract_table_name(sql)
}

fn extract_select_data(sql: &str) -> Option<(String, String)> {
    let sql = sql.trim();

    if sql.to_uppercase().starts_with("SELECT") {
        // 解析表名
        let table_name = extract_table_name(sql)?;

        // 解析条件
        // 简化实现，假设SQL格式为: SELECT * FROM table WHERE condition
        if let Some(where_pos) = sql.to_uppercase().find("WHERE") {
            let condition = &sql[where_pos + 5..]
                .trim()
                .trim_end_matches(';')
                .to_string();
            return Some((table_name, condition.to_string()));
        } else {
            // 没有条件，返回空字符串
            return Some((table_name, String::new()));
        }
    }

    None
}

// 实现DatabaseTrait
impl DatabaseTrait for Database {
    fn execute(&self, sql: &str) -> Result<()> {
        let sql_upper = sql.trim().to_uppercase();

        // 获取表数据的可变引用
        let mut tables = self.tables.lock().unwrap();

        if sql_upper.starts_with("CREATE TABLE") {
            // 创建表
            let table_name = extract_table_name(sql).ok_or(Error::DatabaseError(
                "Invalid CREATE TABLE statement".to_string(),
            ))?;

            if !tables.contains_key(&table_name) {
                tables.insert(table_name.clone(), Vec::new());
            }
        } else if sql_upper.starts_with("INSERT INTO") {
            // 插入数据
            let (table_name, id, data) = extract_insert_data(sql)
                .ok_or(Error::DatabaseError("Invalid INSERT statement".to_string()))?;

            // 获取表
            let table = tables.entry(table_name.clone()).or_insert_with(Vec::new);

            // 查找是否已存在相同ID的记录
            let existing_index = table.iter().position(|row| {
                row.get("id")
                    .map_or(false, |existing_id| existing_id == &id)
            });

            // 创建新记录
            let mut new_row = HashMap::new();
            new_row.insert("id".to_string(), id);
            new_row.insert("data".to_string(), data);

            if let Some(index) = existing_index {
                // 更新现有记录
                table[index] = new_row;
            } else {
                // 添加新记录
                table.push(new_row);
            }
        } else if sql_upper.starts_with("UPDATE") {
            // 更新数据
            let (table_name, id, data) = extract_update_data(sql)
                .ok_or(Error::DatabaseError("Invalid UPDATE statement".to_string()))?;

            // 获取表
            let table = tables.entry(table_name.clone()).or_insert_with(Vec::new);

            // 查找是否已存在相同ID的记录
            let existing_index = table.iter().position(|row| {
                row.get("id")
                    .map_or(false, |existing_id| existing_id == &id)
            });

            if let Some(index) = existing_index {
                // 更新现有记录
                table[index].insert("data".to_string(), data);
            } else {
                // 记录不存在，返回错误
                return Err(Error::NotFound(format!("Record with id {} not found", id)));
            }
        } else if sql_upper.starts_with("DELETE FROM") {
            // 删除数据
            let (table_name, id) = extract_delete_data(sql)
                .ok_or(Error::DatabaseError("Invalid DELETE statement".to_string()))?;

            // 获取表
            if let Some(table) = tables.get_mut(&table_name) {
                // 查找是否已存在相同ID的记录
                let existing_index = table.iter().position(|row| {
                    row.get("id")
                        .map_or(false, |existing_id| existing_id == &id)
                });

                if let Some(index) = existing_index {
                    // 删除记录
                    table.remove(index);
                }
            }
        } else if sql_upper.starts_with("DROP TABLE") {
            // 删除表
            let table_name = extract_drop_table_name(sql).ok_or(Error::DatabaseError(
                "Invalid DROP TABLE statement".to_string(),
            ))?;

            // 删除表
            tables.remove(&table_name);
        }

        Ok(())
    }

    fn query(&self, sql: &str) -> Result<Vec<HashMap<String, String>>> {
        let sql_upper = sql.trim().to_uppercase();

        // 获取表数据的引用
        let tables = self.tables.lock().unwrap();

        if sql_upper.starts_with("SELECT") {
            // 查询数据
            let (table_name, condition) = extract_select_data(sql)
                .ok_or(Error::DatabaseError("Invalid SELECT statement".to_string()))?;

            // 获取表
            if let Some(table) = tables.get(&table_name) {
                // 如果没有条件，返回所有记录
                if condition.is_empty() {
                    return Ok(table.clone());
                }

                // 简化实现，只支持简单的条件查询
                // 例如: id = 'value'
                let parts: Vec<&str> = condition.split('=').collect();
                if parts.len() == 2 {
                    let field = parts[0].trim();
                    let value = parts[1].trim().trim_matches('\'').trim_matches('"');

                    // 过滤记录
                    let filtered: Vec<HashMap<String, String>> = table
                        .iter()
                        .filter(|row| row.get(field).map_or(false, |v| v == value))
                        .cloned()
                        .collect();

                    return Ok(filtered);
                }

                // 不支持的条件，返回所有记录
                return Ok(table.clone());
            }

            // 表不存在，返回空结果
            return Ok(Vec::new());
        }

        // 不支持的SQL，返回错误
        Err(Error::DatabaseError("Unsupported SQL query".to_string()))
    }

    fn get_table(&self, table: &str) -> Result<Option<Vec<HashMap<String, String>>>> {
        // 获取表数据的引用
        let tables = self.tables.lock().unwrap();

        // 获取表
        if let Some(table_data) = tables.get(table) {
            Ok(Some(table_data.clone()))
        } else {
            Ok(None)
        }
    }

    fn get(&self, table: &str, id: &str) -> Result<Option<String>> {
        // 获取表数据的引用
        let tables = self.tables.lock().unwrap();

        // 获取表
        if let Some(table_data) = tables.get(table) {
            // 查找记录
            for row in table_data {
                if let Some(row_id) = row.get("id") {
                    if row_id == id {
                        return Ok(row.get("data").cloned());
                    }
                }
            }
        }

        // 记录不存在
        Ok(None)
    }

    fn insert(&self, table: &str, id: &str, data: &str) -> Result<()> {
        // 构建SQL
        let sql = format!(
            "INSERT INTO {} (id, data) VALUES ('{}', '{}')",
            table, id, data
        );

        // 执行SQL
        self.execute(&sql)
    }

    fn update(&self, table: &str, id: &str, data: &str) -> Result<()> {
        // 构建SQL
        let sql = format!("UPDATE {} SET data = '{}' WHERE id = '{}'", table, data, id);

        // 执行SQL
        self.execute(&sql)
    }

    fn delete(&self, table: &str, id: &str) -> Result<()> {
        // 构建SQL
        let sql = format!("DELETE FROM {} WHERE id = '{}'", table, id);

        // 执行SQL
        self.execute(&sql)
    }
}

// 事务实现
struct TransactionImpl {
    db: Arc<Database>,
    tx_id: String,
    ctx: Option<Context>,
}

impl TransactionImpl {
    // 创建新事务
    fn new(db: &Database, tx_id: String) -> Self {
        Self {
            db: Arc::new(db.clone()),
            tx_id,
            ctx: None,
        }
    }

    // 创建带上下文的事务
    fn with_context(db: &Database, tx_id: String, ctx: Context) -> Self {
        Self {
            db: Arc::new(db.clone()),
            tx_id,
            ctx: Some(ctx),
        }
    }
}

// 实现Transaction特性
impl Transaction for TransactionImpl {
    fn begin(&self) -> Result<()> {
        // 实现事务开始逻辑
        Ok(())
    }

    fn commit(&self) -> Result<()> {
        // 实现事务提交逻辑
        Ok(())
    }

    fn rollback(&self) -> Result<()> {
        // 实现事务回滚逻辑
        Ok(())
    }
}
