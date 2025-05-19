use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields, Lit, Meta, NestedMeta};

/// 自动为结构体实现 Model、Identifiable 和 StateSerializable 特性
///
/// # 参数
///
/// - `table` - 表名，默认为结构体名称的小写形式
/// - `id` - 主键字段名，默认为 "id"
///
/// # 示例
///
/// ```rust
/// #[derive(Debug, Clone, Serialize, Deserialize, Default, mpModel)]
/// #[mp(table = "users", id = "id")]
/// struct User {
///     id: String,
///     name: String,
///     email: String,
/// }
/// ```
#[proc_macro_derive(mpModel, attributes(mp))]
pub fn derive_mp_model(input: TokenStream) -> TokenStream {
    // 解析输入
    let input = parse_macro_input!(input as DeriveInput);

    // 获取结构体名称
    let name = &input.ident;

    // 提取表名和ID字段
    let (table_name, id_field) = extract_mp_attributes(&input.attrs);

    // 确保ID字段存在
    let id_field_exists = if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            fields.named.iter().any(|f| {
                if let Some(ident) = &f.ident {
                    ident.to_string() == id_field.to_string()
                } else {
                    false
                }
            })
        } else {
            false
        }
    } else {
        false
    };

    if !id_field_exists {
        panic!("ID field '{}' not found in struct '{}'", id_field, name);
    }

    // 生成实现代码
    let expanded = quote! {
        // 实现 Model 特性
        impl ::mp_framework::model::Model for #name {
            fn get_table_name() -> &'static str {
                #table_name
            }

            fn get_entity_type() -> &'static str {
                #table_name
            }

            fn get_id(&self) -> String {
                self.#id_field.to_string()
            }

            fn from_json(json: &str) -> ::mp_framework::errors::Result<Self> {
                serde_json::from_str(json)
                    .map_err(|e| ::mp_framework::errors::Error::SerializationError(e.to_string()))
            }

            fn to_json(&self) -> ::mp_framework::errors::Result<String> {
                serde_json::to_string(self)
                    .map_err(|e| ::mp_framework::errors::Error::SerializationError(e.to_string()))
            }
        }

        // 实现 Identifiable 特性
        impl ::mp_framework::state_serialize::Identifiable for #name {
            fn get_id(&self) -> String {
                self.#id_field.to_string()
            }

            fn get_entity_type() -> &'static str {
                #table_name
            }
        }

        // 实现 StateSerializable 特性
        impl ::mp_framework::state_serialize::StateSerializable for #name {
            fn save(&self, ctx: &mut ::mp_framework::context::Context) -> ::mp_framework::errors::Result<()>
            where Self: ::mp_framework::model::Model + ::serde::Serialize {
                ctx.track_entity(self)
            }

            fn delete(&self, ctx: &mut ::mp_framework::context::Context) -> ::mp_framework::errors::Result<()>
            where Self: ::mp_framework::model::Model {
                ctx.delete_entity::<Self>(&<Self as ::mp_framework::state_serialize::Identifiable>::get_id(self))
            }
        }
    };

    // 返回生成的代码
    TokenStream::from(expanded)
}

/// 提取 mp 属性
fn extract_mp_attributes(attrs: &[Attribute]) -> (&str, syn::Ident) {
    let mut table_name = "";
    let mut id_field = None;

    for attr in attrs {
        if attr.path.is_ident("mp") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in meta_list.nested.iter() {
                    if let NestedMeta::Meta(Meta::NameValue(name_value)) = nested {
                        if name_value.path.is_ident("table") {
                            if let Lit::Str(lit_str) = &name_value.lit {
                                table_name = Box::leak(lit_str.value().into_boxed_str());
                            }
                        } else if name_value.path.is_ident("id") {
                            if let Lit::Str(lit_str) = &name_value.lit {
                                id_field = Some(format_ident!("{}", lit_str.value()));
                            }
                        }
                    }
                }
            }
        }
    }

    if table_name.is_empty() {
        panic!("Missing 'table' attribute in #[mp]");
    }

    if id_field.is_none() {
        panic!("Missing 'id' attribute in #[mp]");
    }

    (table_name, id_field.unwrap())
}
