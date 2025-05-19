use crate::context::Context;
use crate::errors::{Error, Result};
use crate::routes::RouteDefinition;
use crate::state_serialize::{Identifiable, StateSerializable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// 合约特性
pub trait Contract {
    // 获取合约名称
    fn name(&self) -> &str;

    // 获取合约版本
    fn version(&self) -> &str;

    // 初始化合约
    fn init(&self, ctx: &mut Context) -> Result<()>;

    // 实现这个方法来允许合约与路由系统集成
    fn register_routes(&self) -> Vec<RouteDefinition> {
        // 默认实现返回空路由集合
        // 合约应该实现 Route trait 来提供实际的路由
        Vec::new()
    }
}

// 状态变化记录
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateDiff {
    pub changes: HashMap<String, String>,
    pub transaction_id: String,
    pub entities: HashMap<String, EntityChange>,
}

// 实体变化类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntityChangeType {
    Create,
    Update,
    Delete,
}

// 实体变化记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChange {
    pub entity_type: String,
    pub entity_id: String,
    pub change_type: EntityChangeType,
    pub data: Option<Value>,
    pub timestamp: u64,
}

// 数据库操作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseOperation {
    Insert,
    Update,
    Delete,
    Query,
}

impl std::fmt::Display for DatabaseOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseOperation::Insert => write!(f, "INSERT"),
            DatabaseOperation::Update => write!(f, "UPDATE"),
            DatabaseOperation::Delete => write!(f, "DELETE"),
            DatabaseOperation::Query => write!(f, "QUERY"),
        }
    }
}

// 数据库操作记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseChange {
    pub table: String,
    pub key: String,
    pub value: String,
    pub operation: DatabaseOperation,
    pub timestamp: u64,
}

// 状态跟踪器
#[derive(Debug, Clone)]
pub struct StateTracker {
    diffs: Arc<Mutex<Vec<StateDiff>>>,
    current_transaction_id: Arc<Mutex<String>>,
    active_transactions: Arc<Mutex<HashMap<String, bool>>>,
    // 实体缓存，按事务ID和实体类型存储
    entity_cache: Arc<Mutex<HashMap<String, HashMap<String, Value>>>>,
}

impl StateTracker {
    // 创建新的状态跟踪器
    pub fn new() -> Self {
        Self {
            diffs: Arc::new(Mutex::new(Vec::new())),
            current_transaction_id: Arc::new(Mutex::new(String::new())),
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            entity_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // 设置当前事务ID
    pub fn set_transaction_id(&self, transaction_id: &str) {
        let mut current = self.current_transaction_id.lock().unwrap();
        *current = transaction_id.to_string();
    }

    // 记录状态变化
    pub fn record_change(&self, key: &str, value: &str) {
        let transaction_id = self.current_transaction_id.lock().unwrap().clone();
        if transaction_id.is_empty() {
            return; // 没有活动事务，不记录变化
        }

        let mut diffs = self.diffs.lock().unwrap();

        // 查找当前事务的状态差异
        let diff = diffs
            .iter_mut()
            .find(|d| d.transaction_id == transaction_id);

        if let Some(diff) = diff {
            // 更新现有状态差异
            diff.changes.insert(key.to_string(), value.to_string());
        } else {
            // 创建新的状态差异
            let mut changes = HashMap::new();
            changes.insert(key.to_string(), value.to_string());

            let new_diff = StateDiff {
                changes,
                transaction_id,
                entities: HashMap::new(),
            };

            diffs.push(new_diff);
        }
    }

    // 设置状态键值
    pub fn set_state_key(&self, transaction_id: &str, key: &str, value: &str) {
        if transaction_id.is_empty() {
            return; // 没有活动事务，不记录变化
        }

        let mut diffs = self.diffs.lock().unwrap();

        // 查找当前事务的状态差异
        let diff = diffs
            .iter_mut()
            .find(|d| d.transaction_id == transaction_id);

        if let Some(diff) = diff {
            // 更新现有状态差异
            diff.changes.insert(key.to_string(), value.to_string());
        } else {
            // 创建新的状态差异
            let mut changes = HashMap::new();
            changes.insert(key.to_string(), value.to_string());

            let new_diff = StateDiff {
                changes,
                transaction_id: transaction_id.to_string(),
                entities: HashMap::new(),
            };

            diffs.push(new_diff);
        }
    }

    // 获取状态键值
    pub fn get_state_key(&self, transaction_id: &str, key: &str) -> Option<String> {
        let diffs = self.diffs.lock().unwrap();

        // 查找当前事务的状态差异
        let diff = diffs.iter().find(|d| d.transaction_id == transaction_id);

        if let Some(diff) = diff {
            diff.changes.get(key).cloned()
        } else {
            None
        }
    }

    // 跟踪实体变更
    pub fn track_entity<T: StateSerializable + Serialize + Identifiable>(
        &self,
        transaction_id: &str,
        entity: &T,
    ) -> Result<()> {
        if transaction_id.is_empty() {
            return Ok(()); // 没有活动事务，不跟踪实体
        }

        // 序列化实体
        let entity_data =
            serde_json::to_value(entity).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 获取实体类型和ID
        let entity_type = T::get_entity_type();
        let entity_id = entity.get_id();

        // 更新实体缓存
        let mut entity_cache = self.entity_cache.lock().unwrap();
        let tx_cache = entity_cache
            .entry(transaction_id.to_string())
            .or_insert_with(HashMap::new);

        let cache_key = format!("{}/{}", entity_type, entity_id);
        let is_update = tx_cache.contains_key(&cache_key);
        tx_cache.insert(cache_key, entity_data.clone());

        // 更新状态差异
        let mut diffs = self.diffs.lock().unwrap();

        // 查找当前事务的状态差异
        let diff = diffs
            .iter_mut()
            .find(|d| d.transaction_id == transaction_id);

        let change_type = if is_update {
            EntityChangeType::Update
        } else {
            EntityChangeType::Create
        };

        let entity_change = EntityChange {
            entity_type: entity_type.to_string(),
            entity_id: entity_id.clone(),
            change_type,
            data: Some(entity_data),
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        if let Some(diff) = diff {
            // 更新现有状态差异
            diff.entities
                .insert(format!("{}/{}", entity_type, entity_id), entity_change);
        } else {
            // 创建新的状态差异
            let mut entities = HashMap::new();
            entities.insert(format!("{}/{}", entity_type, entity_id), entity_change);

            let new_diff = StateDiff {
                changes: HashMap::new(),
                transaction_id: transaction_id.to_string(),
                entities,
            };

            diffs.push(new_diff);
        }

        Ok(())
    }

    // 跟踪实体变更 - 适配器模式版本
    pub fn track_entity_change(
        &self,
        transaction_id: &str,
        entity_type: &str,
        entity_id: &str,
        entity_data: &serde_json::Value,
    ) -> Result<()> {
        if transaction_id.is_empty() {
            return Ok(()); // 没有活动事务，不跟踪实体
        }

        // 更新实体缓存
        let mut entity_cache = self.entity_cache.lock().unwrap();
        let tx_cache = entity_cache
            .entry(transaction_id.to_string())
            .or_insert_with(HashMap::new);

        let cache_key = format!("{}/{}", entity_type, entity_id);
        let is_update = tx_cache.contains_key(&cache_key);
        tx_cache.insert(cache_key.clone(), entity_data.clone());

        // 更新状态差异
        let mut diffs = self.diffs.lock().unwrap();

        // 查找当前事务的状态差异
        let diff = diffs
            .iter_mut()
            .find(|d| d.transaction_id == transaction_id);

        let change_type = if is_update {
            EntityChangeType::Update
        } else {
            EntityChangeType::Create
        };

        let entity_change = EntityChange {
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            change_type,
            data: Some(entity_data.clone()),
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        if let Some(diff) = diff {
            // 更新现有状态差异
            diff.entities.insert(cache_key, entity_change);
        } else {
            // 创建新的状态差异
            let mut entities = HashMap::new();
            entities.insert(cache_key, entity_change);

            let new_diff = StateDiff {
                changes: HashMap::new(),
                transaction_id: transaction_id.to_string(),
                entities,
            };

            diffs.push(new_diff);
        }

        Ok(())
    }

    // 跟踪实体删除
    pub fn delete_entity<T: Identifiable>(&self, transaction_id: &str, entity_id: &str) {
        if transaction_id.is_empty() {
            return; // 没有活动事务，不跟踪实体
        }

        // 获取实体类型
        let entity_type = T::get_entity_type();

        // 更新实体缓存
        let mut entity_cache = self.entity_cache.lock().unwrap();
        if let Some(tx_cache) = entity_cache.get_mut(transaction_id) {
            let cache_key = format!("{}/{}", entity_type, entity_id);
            tx_cache.remove(&cache_key);
        }

        // 更新状态差异
        let mut diffs = self.diffs.lock().unwrap();

        // 查找当前事务的状态差异
        let diff = diffs
            .iter_mut()
            .find(|d| d.transaction_id == transaction_id);

        let entity_change = EntityChange {
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            change_type: EntityChangeType::Delete,
            data: None,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        if let Some(diff) = diff {
            // 更新现有状态差异
            let entity_key = format!("{}/{}", entity_type, entity_id);
            diff.entities.insert(entity_key, entity_change);
        } else {
            // 创建新的状态差异
            let mut entities = HashMap::new();
            let entity_key = format!("{}/{}", entity_type, entity_id);
            entities.insert(entity_key, entity_change);

            let new_diff = StateDiff {
                changes: HashMap::new(),
                transaction_id: transaction_id.to_string(),
                entities,
            };

            diffs.push(new_diff);
        }
    }

    // 从缓存中获取实体
    pub fn get_entity<T: StateSerializable + Identifiable + for<'de> serde::Deserialize<'de>>(
        &self,
        transaction_id: &str,
        entity_id: &str,
    ) -> Result<Option<T>> {
        if transaction_id.is_empty() {
            return Ok(None); // 没有活动事务
        }

        // 获取实体类型
        let entity_type = T::get_entity_type();

        // 从实体缓存中获取
        let entity_cache = self.entity_cache.lock().unwrap();
        if let Some(tx_cache) = entity_cache.get(transaction_id) {
            let cache_key = format!("{}/{}", entity_type, entity_id);
            if let Some(entity_data) = tx_cache.get(&cache_key) {
                // 反序列化实体
                let entity = serde_json::from_value(entity_data.clone())
                    .map_err(|e| Error::SerializationError(e.to_string()))?;
                return Ok(Some(entity));
            }
        }

        Ok(None)
    }

    // 获取特定事务的状态差异
    pub fn get_diff(&self, transaction_id: &str) -> Option<StateDiff> {
        let diffs = self.diffs.lock().unwrap();
        diffs
            .iter()
            .find(|d| d.transaction_id == transaction_id)
            .cloned()
    }

    // 获取所有状态差异
    pub fn get_all_diffs(&self) -> Vec<StateDiff> {
        self.diffs.lock().unwrap().clone()
    }

    // 清除特定事务的状态差异
    pub fn clear_diff(&self, transaction_id: &str) {
        let mut diffs = self.diffs.lock().unwrap();
        diffs.retain(|d| d.transaction_id != transaction_id);
    }

    // 开始事务
    pub fn begin_transaction(&self, transaction_id: &str) {
        let mut active = self.active_transactions.lock().unwrap();
        active.insert(transaction_id.to_string(), true);

        // 设置当前事务ID
        self.set_transaction_id(transaction_id);
    }

    // 结束事务并返回状态差异
    pub fn end_transaction(&self, transaction_id: &str) -> Option<StateDiff> {
        // 标记事务完成
        let mut active = self.active_transactions.lock().unwrap();
        active.remove(transaction_id);

        // 获取并返回状态差异
        let diff = self.get_diff(transaction_id);

        // 重置当前事务ID（如果它是当前活动的事务）
        let current = self.current_transaction_id.lock().unwrap().clone();
        if current == transaction_id {
            self.set_transaction_id("");
        }

        diff
    }

    // 取消事务
    pub fn cancel_transaction(&self, transaction_id: &str) {
        // 标记事务取消
        let mut active = self.active_transactions.lock().unwrap();
        active.remove(transaction_id);

        // 清除状态差异
        self.clear_diff(transaction_id);

        // 重置当前事务ID（如果它是当前活动的事务）
        let current = self.current_transaction_id.lock().unwrap().clone();
        if current == transaction_id {
            self.set_transaction_id("");
        }
    }

    // 检查事务是否活动
    pub fn is_transaction_active(&self, transaction_id: &str) -> bool {
        let active = self.active_transactions.lock().unwrap();
        active.get(transaction_id).copied().unwrap_or(false)
    }
}
