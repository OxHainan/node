// 这个模块提供了实用宏

// 实现模型特性的宏
#[macro_export]
macro_rules! impl_model {
    ($type:ty, $table_name:expr, $id_field:ident) => {
        impl $crate::model::Model for $type {
            fn get_table_name() -> &'static str {
                $table_name
            }

            fn get_id(&self) -> String {
                self.$id_field.clone()
            }

            fn from_json(json: &str) -> $crate::errors::Result<Self> {
                match serde_json::from_str(json) {
                    Ok(model) => Ok(model),
                    Err(e) => Err($crate::errors::Error::SerializationError(format!(
                        "Failed to deserialize model: {}",
                        e
                    ))),
                }
            }

            fn to_json(&self) -> $crate::errors::Result<String> {
                match serde_json::to_string(self) {
                    Ok(json) => Ok(json),
                    Err(e) => Err($crate::errors::Error::SerializationError(format!(
                        "Failed to serialize model: {}",
                        e
                    ))),
                }
            }
        }

        // 同时实现状态实体特性
        $crate::impl_state_entity!($type, $table_name, $id_field);
    };
}

// 实现状态实体特性的宏
#[macro_export]
macro_rules! impl_state_entity {
    ($type:ty, $table_name:expr, $id_field:ident) => {
        impl $crate::state_serialize::Identifiable for $type {
            fn get_id(&self) -> String {
                self.$id_field.clone()
            }

            fn get_entity_type() -> &'static str {
                $table_name
            }
        }

        impl $crate::state_serialize::StateSerializable for $type {
            fn to_state_entries(&self) -> Vec<(String, String)> {
                let mut entries = Vec::new();

                // 主键-值对
                let primary_key = format!("{}:{}", Self::get_entity_type(), self.get_id());
                if let Ok(json) = serde_json::to_string(self) {
                    entries.push((primary_key.clone(), json));

                    // 索引键值对 - 类型索引
                    let type_index_key =
                        format!("{}_index:{}", Self::get_entity_type(), self.get_id());
                    entries.push((type_index_key, "1".to_string()));
                }

                entries
            }

            fn from_state_entries(entries: &[(String, String)]) -> $crate::errors::Result<Self> {
                for (key, value) in entries {
                    // 匹配主键模式
                    let prefix = format!("{}:", Self::get_entity_type());
                    if key.starts_with(&prefix) {
                        // 尝试反序列化
                        return Self::from_json(value);
                    }
                }

                Err($crate::errors::Error::NotFound(format!(
                    "Entity of type {} not found",
                    Self::get_entity_type()
                )))
            }

            fn get_primary_key(&self) -> Option<String> {
                Some(format!("{}:{}", Self::get_entity_type(), self.get_id()))
            }
        }
    };
}
