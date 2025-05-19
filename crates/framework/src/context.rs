use crate::database::Database;
use crate::errors::{Error, Result};
use crate::model::Model;
use crate::state::StateTracker;
use crate::state_serialize::{Identifiable, StateSerializable};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// 状态变更
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// 实体变更
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDiff {
    pub entity_type: String,
    pub id: String,
    pub action: String,
    pub data: Option<serde_json::Value>,
}

/// 事务上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// 事务ID
    pub transaction_id: String,

    /// 状态变更
    pub state_diffs: Vec<StateDiff>,

    /// 实体变更
    pub entity_diffs: Vec<EntityDiff>,

    /// 当前状态
    #[serde(skip)]
    state: HashMap<String, String>,

    /// 已删除的实体
    #[serde(skip)]
    deleted_entities: HashSet<String>,

    /// 数据库引用
    #[serde(skip)]
    database: Option<Arc<Database>>,

    /// 状态跟踪是否启用
    #[serde(skip)]
    state_tracking_enabled: bool,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    /// 创建新的上下文
    pub fn new() -> Self {
        Self {
            transaction_id: Uuid::new_v4().to_string(),
            state_diffs: Vec::new(),
            entity_diffs: Vec::new(),
            state: HashMap::new(),
            deleted_entities: HashSet::new(),
            database: None,
            state_tracking_enabled: false,
        }
    }

    /// 创建带数据库的上下文
    pub fn with_database(db: Arc<Database>) -> Self {
        let mut ctx = Self::new();
        ctx.database = Some(db);
        ctx
    }

    /// 获取数据库引用
    pub fn db(&self) -> Arc<Database> {
        self.database.clone().expect("Database not set in context")
    }

    /// 启用状态跟踪
    pub fn enable_state_tracking(&mut self) {
        self.state_tracking_enabled = true;
    }

    /// 禁用状态跟踪
    pub fn disable_state_tracking(&mut self) {
        self.state_tracking_enabled = false;
    }

    /// 设置事务ID
    pub fn set_transaction_id(&mut self, transaction_id: &str) {
        self.transaction_id = transaction_id.to_string();
    }

    /// 使用事务ID
    pub fn with_transaction(&mut self, transaction_id: &str) {
        self.transaction_id = transaction_id.to_string();
    }

    /// 获取状态差异
    pub fn get_diff(&self) -> StateDiff {
        // 返回状态差异
        StateDiff {
            key: "state".to_string(),
            old_value: None,
            new_value: Some(serde_json::to_string(&self.state).unwrap_or_default()),
        }
    }

    /// 取消事务
    pub fn cancel_transaction(&mut self) {
        self.state_diffs.clear();
        self.entity_diffs.clear();
    }

    /// 设置状态键值
    pub fn set_state_key(&mut self, key: &str, value: &str) {
        let old_value = self.state.get(key).cloned();
        let new_value = Some(value.to_string());

        // 只有当值发生变化时才记录
        if old_value != new_value {
            self.state_diffs.push(StateDiff {
                key: key.to_string(),
                old_value,
                new_value: Some(value.to_string()),
            });

            self.state.insert(key.to_string(), value.to_string());
        }
    }

    /// 删除状态键值
    pub fn delete_state_key(&mut self, key: &str) {
        let old_value = self.state.get(key).cloned();

        if old_value.is_some() {
            self.state_diffs.push(StateDiff {
                key: key.to_string(),
                old_value,
                new_value: None,
            });

            self.state.remove(key);
        }
    }

    /// 获取状态键值
    pub fn get_state_key(&self, key: &str) -> Option<&String> {
        self.state.get(key)
    }

    /// 跟踪实体变更
    pub fn track_entity<T>(&mut self, entity: &T) -> Result<()>
    where
        T: Model + StateSerializable + Identifiable + Serialize,
    {
        let entity_type = <T as Model>::get_entity_type();
        let id = <T as Identifiable>::get_id(entity);

        // 如果实体已被删除，则不再跟踪
        let entity_key = format!("{}/{}", entity_type, id);
        if self.deleted_entities.contains(&entity_key) {
            return Ok(());
        }

        // 序列化实体
        let data =
            serde_json::to_value(entity).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 记录实体变更
        self.entity_diffs.push(EntityDiff {
            entity_type: entity_type.to_string(),
            id,
            action: "update".to_string(),
            data: Some(data),
        });

        Ok(())
    }

    /// 记录实体删除
    pub fn delete_entity<T>(&mut self, id: &str) -> Result<()>
    where
        T: Model,
    {
        let entity_type = <T as Model>::get_entity_type();

        // 标记实体已删除
        let entity_key = format!("{}/{}", entity_type, id);
        self.deleted_entities.insert(entity_key);

        // 记录实体删除
        self.entity_diffs.push(EntityDiff {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            action: "delete".to_string(),
            data: None,
        });

        Ok(())
    }

    /// 合并上下文
    pub fn merge(&mut self, other: Context) {
        self.state_diffs.extend(other.state_diffs);
        self.entity_diffs.extend(other.entity_diffs);

        // 合并状态
        for (key, value) in other.state {
            self.state.insert(key, value);
        }

        // 合并已删除实体
        for entity in other.deleted_entities {
            self.deleted_entities.insert(entity);
        }
    }

    /// 清空上下文
    pub fn clear(&mut self) {
        self.state_diffs.clear();
        self.entity_diffs.clear();
        self.state.clear();
        self.deleted_entities.clear();
        self.transaction_id = Uuid::new_v4().to_string();
    }
}
