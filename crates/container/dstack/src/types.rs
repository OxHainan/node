use std::fmt;
use ethereum_types::H128;
use mp_common::utils::{h128_to_uuid, string_to_uuid};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use uuid::Uuid;

const DEFAULT_MEMORY: u16 = 2;
const DEFAULT_VCPUS: u16 = 1;
const DEFAULT_STORAGE: u16 = 10;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CreateVmRequest {
    pub agent_name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub action: CreateAction,
    pub authorization_type: AuthorizationType,
    #[serde(flatten)]
    pub pricing_and_access: PricingAndAccess,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub enum PricingModel {
    #[default]
    Free,
    PerAPICall(f32),
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub enum AccessControl {
    #[default]
    Public,
    Private,
    Restricted,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PricingAndAccess {
    pub pricing: Option<PricingModel>,
    pub daily_call_quote: u16,
    pub access: Option<AccessControl>,
}

impl Default for PricingAndAccess {
    fn default() -> Self {
        Self {
            pricing: Some(PricingModel::Free),
            daily_call_quote: 100,
            access: Some(AccessControl::Public),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum CreateAction {
    Agent(AgentConfiguration),
    External(HostingExternal),
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub enum AuthorizationType {
    #[default]
    None,
    APIKEY,
    OAuth2,
    JWT,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AgentConfiguration {
    pub name: String,
    pub docker_compose: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub encrypted_env: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    v_cpus: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    memory: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    storage: Option<u16>,
    pub path: String,
}

impl AgentConfiguration {
    pub fn memory(&self) -> u16 {
        self.memory.unwrap_or(DEFAULT_MEMORY)
    }

    pub fn v_cpus(&self) -> u16 {
        self.v_cpus.unwrap_or(DEFAULT_VCPUS)
    }

    pub fn storage(&self) -> u16 {
        self.storage.unwrap_or(DEFAULT_STORAGE)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HostingExternal {
    pub domain: String,
    pub protocol: EndpointProtocol,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub enum EndpointProtocol {
    #[default]
    Http,
    Https,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RequestId {
    pub id: VmId,
}

impl RequestId {
    pub fn new(id: VmId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> Uuid {
        h128_to_uuid(&self.id.id())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmId {
    Name(String),
    Id(H128),
}

impl VmId {
    pub fn is_name(&self) -> bool {
        matches!(self, VmId::Name(_))
    }

    pub fn is_id(&self) -> bool {
        matches!(self, VmId::Id(_))
    }

    pub fn id(&self) -> H128 {
        match self {
            VmId::Id(id) => *id,
            VmId::Name(name) => string_to_uuid(Some(name.clone())).as_bytes().into(),
        }
    }
}

// 手动实现 Serialize
impl Serialize for VmId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            VmId::Name(name) => serializer.serialize_str(name),
            VmId::Id(hid) => serializer.serialize_str(&format!("0x{}", hex::encode(hid.0))),
        }
    }
}

// 手动实现 Deserialize
impl<'de> Deserialize<'de> for VmId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VmIdVisitor;

        impl<'de> Visitor<'de> for VmIdVisitor {
            type Value = VmId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a string starting with 0x (for Id) or normal string (for Name)")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Some(hex) = v.strip_prefix("0x") {
                    let bytes = hex::decode(hex).map_err(de::Error::custom)?;
                    if bytes.len() != 16 {
                        return Err(de::Error::custom("H128 must be 16 bytes"));
                    }
                    let mut arr = [0u8; 16];
                    arr.copy_from_slice(&bytes);
                    Ok(VmId::Id(H128(arr)))
                } else {
                    Ok(VmId::Name(v.to_string()))
                }
            }
        }

        deserializer.deserialize_str(VmIdVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{self, json};

    #[test]
    fn test_vm_id() {
        let vm_id = VmId::Id(H128::from([0; 16]));
        let serialized = serde_json::to_string(&vm_id).unwrap();
        assert_eq!(
            serialized,
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
        let deserialized: VmId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, vm_id);
    }

    #[test]
    fn test_vm_id_name() {
        let vm_id = VmId::Name("test".to_string());
        let id = vm_id.id();
        println!("id: {}", id);
        let id = RequestId { id: vm_id };
        println!("id: {}", id.id());
    }

    #[test]
    fn test_create_vm_request() {
        let req = AgentConfiguration {
            docker_compose: "version: '3'\nservices:\n  openai_proxy:\n    image: mpnetwork/openai_proxy:latest\n    ports:\n    - 8100:8100\n    restart: always\n    environment: {}\n".to_string(),
            app_id: None,
            encrypted_env: vec![],
            path: "test".to_string(),
            name: "test".to_string(),
            ..Default::default()
        };
        let serialized = serde_json::to_string_pretty(&req).unwrap();
        println!("Serialized:  {}", serialized);
        println!("{:?}", req.storage());
        let req1 = CreateVmRequest {
            agent_name: "test".to_string(),
            description: "test".to_string(),
            action: CreateAction::Agent(req),
            authorization_type: AuthorizationType::APIKEY,
            tags: vec![],
            pricing_and_access: PricingAndAccess::default(),
        };
        let serialized = serde_json::to_string_pretty(&req1).unwrap();
        println!("Serialized: {}", serialized);
        let deserialized: CreateVmRequest = serde_json::from_str(&serialized).unwrap();
        println!("Deserialized: {:#?}", deserialized);

        let req2 = CreateVmRequest {
            agent_name: "test".to_string(),
            description: "test".to_string(),
            action: CreateAction::External(HostingExternal {
                domain: "test".to_string(),
                protocol: EndpointProtocol::Http,
            }),
            authorization_type: AuthorizationType::APIKEY,
            tags: vec![],
            pricing_and_access: PricingAndAccess::default(),
        };
        let serialized = serde_json::to_string_pretty(&req2).unwrap();
        println!("Serialized: {}", serialized);
        let deserialized: CreateVmRequest = serde_json::from_str(&serialized).unwrap();
        println!("Deserialized: {:#?}", deserialized);
    }

    #[test]
    fn test_create_vm_request_with_json() {
        let json1 = json!({
            "agent_name": "test",
            "description": "test",
            "docker_compose": "version: '3'\nservices:\n  openai_proxy:\n    image: mpnetwork/openai_proxy:latest\n    ports:\n    - 8100:8100\n    restart: always\n    environment: {}\n",
            "path": "test",
            "authorization_type": "APIKEY",
            "name": "test",
            "tags": [],
            "daily_call_quote": 100,
        });
        let deserialized: CreateVmRequest = serde_json::from_value(json1).unwrap();
        println!("Deserialized: {:#?}", deserialized);
        let json1 = json!({
            "agent_name": "test",
            "description": "test",
            "name": "test",
            "domain": "test",
            "protocol": "Http",
            "authorization_type": "APIKEY",
            "daily_call_quote": 100,
        });
        let deserialized: CreateVmRequest = serde_json::from_value(json1).unwrap();
        println!("Deserialized: {:#?}", deserialized);
    }
}
