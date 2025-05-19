use anyhow::Result;
use compose::{DockerCompose, PortMapping as DockerPortMapping};
use dstack_types::AppCompose;
use guest_api::proxied_guest_api_client::ProxiedGuestApiClient;
use guest_api::Id as GuestId;
use http_client::prpc::PrpcClient;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
pub use tappd_rpc::{
    tappd_client, DeriveK256KeyArgs, DeriveK256KeyResponse, DeriveKeyArgs, DeriveKeyResponse,
    RawQuoteArgs, TdxQuoteArgs, TdxQuoteResponse, WorkerInfo,
};
pub use teepod_rpc::Id;
use teepod_rpc::{
    teepod_client::TeepodClient, GetInfoResponse, GpuSpec, ImageListResponse, PortMapping,
    StatusResponse, VmConfiguration as TeepodVmConfiguration, VmInfo as TeepodVmInfo,
};
use tracing::info;
use types::AgentConfiguration;
use utils::uuid_to_id;
use uuid::Uuid;

pub mod compose;
pub mod types;

pub struct PodClient {
    client: TeepodClient<PrpcClient>,
    tappd_client: TappdClient,
    guest_client: ProxiedGuestApiClient<PrpcClient>,
    pub vms: Arc<Mutex<BTreeMap<String, VmInfo>>>,
}

pub struct TappdClient {
    client: tappd_client::TappdClient<PrpcClient>,
}

impl TappdClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let url = base_url.into();
        info!("Using tappd socket: {}", url);
        if url.starts_with("/var/run") {
            if !std::fs::exists(&url).unwrap_or_default() {
                panic!("tappd socket({}) not found", url);
            }
            Self {
                client: tappd_client::TappdClient::new(PrpcClient::new_unix(
                    url,
                    "prpc".to_string(),
                )),
            }
        } else {
            Self {
                client: tappd_client::TappdClient::new(PrpcClient::new(format!("{}/prpc", url))),
            }
        }
    }
}

impl PodClient {
    pub async fn new(base_url: impl Into<String>, tappd_url: impl Into<String>) -> Result<Self> {
        let url = base_url.into();

        let this = Self {
            vms: Arc::new(Mutex::new(BTreeMap::new())),
            client: TeepodClient::new(PrpcClient::new(format!("{}/prpc", url))),
            tappd_client: TappdClient::new(tappd_url),
            guest_client: ProxiedGuestApiClient::new(PrpcClient::new(format!("{}/guest", url))),
        };

        this.init_vms().await?;
        Ok(this)
    }

    async fn init_vms(&self) -> Result<()> {
        let vms = self
            .get_vm_info()
            .await?
            .into_iter()
            .map(|info| (info.name.clone(), info))
            .collect::<BTreeMap<_, _>>();
        self.vms.lock().extend(vms);
        Ok(())
    }

    pub fn get_vms(&self) -> Result<Vec<VmInfo>> {
        let vms = self.vms.lock().clone();
        Ok(vms.into_iter().map(|(_, vm)| vm).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub id: Uuid,
    pub name: String,
    pub status: Status,
    pub instance_id: String,
    pub app_id: String,
    pub ip: IpAddr,
    pub port: DockerPortMapping,
}

impl VmInfo {
    pub fn base_url(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port.host_port)
    }
}

impl TryFrom<(TeepodVmInfo, IpAddr)> for VmInfo {
    type Error = anyhow::Error;
    fn try_from(value: (TeepodVmInfo, IpAddr)) -> Result<Self, Self::Error> {
        let port = if let Some(config) = value.0.configuration {
            let app_compose: AppCompose = serde_json::from_str(&config.compose_file)?;
            if let Some(docker_compose_file) = app_compose.docker_compose_file {
                let docker_compose = DockerCompose::from_str(&docker_compose_file)?;
                let ports = docker_compose
                    .services
                    .into_iter()
                    .map(|(_, s)| s.ports.clone())
                    .collect::<Vec<_>>();

                if ports.is_empty() || ports.len() > 1 {
                    return Err(anyhow::anyhow!("Compose file must have at least one port"));
                }
                let ports = ports[0].clone();
                if ports.is_empty() || ports.len() > 1 {
                    return Err(anyhow::anyhow!(
                        "Docker Compose service only has one external port"
                    ));
                }
                ports[0].clone()
            } else {
                return Err(anyhow::anyhow!("Docker compose file not found"));
            }
        } else {
            return Err(anyhow::anyhow!("VM configuration not found"));
        };

        Ok(Self {
            id: value.0.id.parse()?,
            name: value.0.name,
            status: value.0.status.parse()?,
            instance_id: value.0.instance_id.unwrap_or_default(),
            app_id: value.0.app_id,
            ip: value.1,
            port,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Stopped,
    Exited,
    Stopping,
}

impl FromStr for Status {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 为了忽略大小写，先转换成小写再匹配
        match s.to_lowercase().as_str() {
            "running" => Ok(Status::Running),
            "stopped" => Ok(Status::Stopped),
            "exited" => Ok(Status::Exited),
            "stopping" => Ok(Status::Stopping),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid status",
            )),
        }
    }
}

#[async_trait::async_trait]
pub trait PodClientT {
    async fn create_vm(&self, req: AgentConfiguration) -> Result<VmInfo>;
    async fn start_vm(&self, id: &Uuid) -> Result<()>;
    async fn stop_vm(&self, id: &Uuid) -> Result<()>;
    async fn remove_vm(&self, id: &Uuid) -> Result<()>;
    async fn status(&self) -> Result<StatusResponse>;
    async fn list_images(&self) -> Result<ImageListResponse>;
    async fn get_info(&self, id: Id) -> Result<GetInfoResponse>;
    async fn get_network_ip(&self, id: String) -> Result<IpAddr>;
    async fn get_vm_info(&self) -> Result<Vec<VmInfo>>;
}

#[async_trait::async_trait]
pub trait TappdClientT: Send + Sync + 'static {
    async fn derive_key(&self, args: DeriveKeyArgs) -> Result<DeriveKeyResponse>;
    async fn derive_k256_key(&self, args: DeriveK256KeyArgs) -> Result<DeriveK256KeyResponse>;
    async fn tdx_quote(&self, args: TdxQuoteArgs) -> Result<TdxQuoteResponse>;
    async fn raw_quote(&self, args: RawQuoteArgs) -> Result<TdxQuoteResponse>;
    async fn info(&self) -> Result<WorkerInfo>;
}

#[async_trait::async_trait]
impl TappdClientT for TappdClient {
    async fn derive_key(&self, args: DeriveKeyArgs) -> Result<DeriveKeyResponse> {
        self.client.derive_key(args).await
    }
    async fn derive_k256_key(&self, args: DeriveK256KeyArgs) -> Result<DeriveK256KeyResponse> {
        self.client.derive_k256_key(args).await
    }
    async fn tdx_quote(&self, args: TdxQuoteArgs) -> Result<TdxQuoteResponse> {
        self.client.tdx_quote(args).await
    }
    async fn raw_quote(&self, args: RawQuoteArgs) -> Result<TdxQuoteResponse> {
        self.client.raw_quote(args).await
    }
    async fn info(&self) -> Result<WorkerInfo> {
        self.client.info().await
    }
}

#[async_trait::async_trait]
impl TappdClientT for PodClient {
    async fn derive_key(&self, args: DeriveKeyArgs) -> Result<DeriveKeyResponse> {
        self.tappd_client.derive_key(args).await
    }
    async fn derive_k256_key(&self, args: DeriveK256KeyArgs) -> Result<DeriveK256KeyResponse> {
        self.tappd_client.derive_k256_key(args).await
    }
    async fn tdx_quote(&self, args: TdxQuoteArgs) -> Result<TdxQuoteResponse> {
        self.tappd_client.tdx_quote(args).await
    }
    async fn raw_quote(&self, args: RawQuoteArgs) -> Result<TdxQuoteResponse> {
        self.tappd_client.raw_quote(args).await
    }
    async fn info(&self) -> Result<WorkerInfo> {
        self.tappd_client.info().await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfiguration {
    pub compose_file: AppCompose,
    pub image: String,
    pub vcpu: u32,
    pub memory: u32,
    pub disk_size: u32,
    #[serde(default)]
    pub ports: Vec<PortMapping>,
    #[serde(default)]
    pub encrypted_env: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub user_config: String,
    #[serde(default)]
    pub hugepages: bool,
    #[serde(default)]
    pub pin_numa: bool,
    #[serde(default)]
    pub gpus: Vec<GpuSpec>,
}

impl Default for VmConfiguration {
    fn default() -> Self {
        Self {
            compose_file: AppCompose {
                manifest_version: 2,
                name: Default::default(),
                features: Default::default(),
                runner: "docker-compose".to_string(),
                docker_compose_file: None,
                docker_config: Default::default(),
                public_logs: true,
                public_sysinfo: true,
                kms_enabled: true,
                tproxy_enabled: true,
                local_key_provider_enabled: false,
                key_provider: None,
                allowed_envs: Default::default(),
                no_instance_id: false,
            },
            image: "dstack-dev-0.4.1".to_string(),
            vcpu: 2,
            memory: 1024,
            disk_size: 20,
            ports: Default::default(),
            encrypted_env: Default::default(),
            app_id: None,
            user_config: Default::default(),
            hugepages: false,
            pin_numa: false,
            gpus: Default::default(),
        }
    }
}

impl TryFrom<VmConfiguration> for TeepodVmConfiguration {
    type Error = anyhow::Error;
    fn try_from(req: VmConfiguration) -> Result<Self, Self::Error> {
        let compose_file = serde_json::to_string(&req.compose_file)?;

        Ok(Self {
            name: req.compose_file.name,
            image: req.image,
            compose_file,
            vcpu: req.vcpu,
            memory: req.memory,
            disk_size: req.disk_size,
            ports: req.ports,
            encrypted_env: req.encrypted_env,
            app_id: req.app_id,
            user_config: req.user_config,
            hugepages: req.hugepages,
            pin_numa: req.pin_numa,
            gpus: req.gpus,
        })
    }
}

impl From<AgentConfiguration> for VmConfiguration {
    fn from(req: AgentConfiguration) -> Self {
        let mut config = Self::default();
        config.compose_file.docker_compose_file = Some(req.docker_compose);
        config.compose_file.name = req.name;
        config.app_id = req.app_id;
        config.encrypted_env = req.encrypted_env;
        config
    }
}

impl VmConfiguration {
    pub fn validate(&self) -> Result<()> {
        // 检查 compose_file 是否存在
        if let Some(docker_compose_file) = &self.compose_file.docker_compose_file {
            let docker_compose = DockerCompose::from_str(docker_compose_file)?;
            if docker_compose.services.is_empty() {
                return Err(anyhow::anyhow!(
                    "Compose file must have at least one service"
                ));
            }

            // 验证只能存在一个外部服务
            let ports = docker_compose
                .services
                .iter()
                .map(|(_, s)| s.ports.clone())
                .collect::<Vec<_>>();
            if ports.is_empty() || ports.len() > 1 {
                return Err(anyhow::anyhow!("Compose file must have at least one port"));
            }

            let ports = &ports[0];
            if ports.is_empty() || ports.len() > 1 {
                return Err(anyhow::anyhow!(
                    "Docker Compose service only has one external port"
                ));
            }

            let DockerPortMapping {
                host_port,
                container_port,
            } = ports[0];
            if host_port <= 1000 {
                // 防止随意开放系统端口
                return Err(anyhow::anyhow!(
                    "Docker host port must be greater than 1000"
                ));
            }

            if !(container_port >= 1000 || container_port == 80 || container_port == 443) {
                // 防止随意开放系统端口
                return Err(anyhow::anyhow!(
                    "Docker container port must be greater than 1000"
                ));
            }
        } else {
            return Err(anyhow::anyhow!("Compose file is required"));
        }

        // 检查name 是否存在
        if self.compose_file.name.is_empty() {
            return Err(anyhow::anyhow!("CVM Name is required"));
        }

        // 检查 image 是否存在
        if self.image.is_empty() {
            return Err(anyhow::anyhow!("Image is required"));
        }
        // 检查 vcpu 是否在合理范围内
        if self.vcpu < 1 || self.vcpu > 32 {
            return Err(anyhow::anyhow!("vcpu must be between 1 and 32"));
        }
        // 检查 memory 是否在合理范围内
        if self.memory < 1024 || self.memory > 1024 * 6 {
            return Err(anyhow::anyhow!("memory must be between 1024 and 6144"));
        }
        // 检查 disk_size 是否在合理范围内
        if self.disk_size < 10 || self.disk_size > 100 {
            return Err(anyhow::anyhow!("disk_size must be between 10 and 100"));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl PodClientT for PodClient {
    async fn create_vm(&self, req: AgentConfiguration) -> Result<VmInfo> {
        // 检查name是否已存在
        if self.vms.lock().contains_key(&req.name) {
            return Err(anyhow::anyhow!("CVM Name already exists"));
        }

        let config = VmConfiguration::try_from(req)?;
        config.validate()?;
        let id = self.client.create_vm(config.try_into()?).await?;

        let max_attempts = 150; // 最大尝试次数
        let wait_interval = std::time::Duration::from_secs(5); // 每次等待5秒

        for attempt in 1..=max_attempts {
            let info_response = self.get_info(id.clone()).await?;

            if let Some(info) = info_response.info {
                if info.instance_id.is_some() {
                    // 虚拟机创建成功且初始化完成
                    tracing::info!(
                        "VM successfully created and initialized after {} attempts",
                        attempt
                    );

                    let ip = self.get_network_ip(id.id.clone()).await?;
                    let vm: VmInfo = (info, ip).try_into()?;
                    self.vms.lock().insert(vm.name.clone(), vm.clone());
                    return Ok(vm);
                }

                if !info.boot_error.is_empty() {
                    return Err(anyhow::anyhow!("VM creation failed: {}", info.boot_error));
                }
            }

            tracing::info!(
                "Waiting for VM to initialize, attempt {}/{}",
                attempt,
                max_attempts
            );
            tokio::time::sleep(wait_interval).await;
        }
        Err(anyhow::anyhow!("Timeout waiting for VM to initialize"))
    }

    async fn start_vm(&self, id: &Uuid) -> Result<()> {
        self.client.start_vm(uuid_to_id(id)).await
    }
    async fn stop_vm(&self, id: &Uuid) -> Result<()> {
        self.client.stop_vm(uuid_to_id(id)).await
    }
    async fn remove_vm(&self, id: &Uuid) -> Result<()> {
        self.client.remove_vm(uuid_to_id(id)).await
    }
    async fn status(&self) -> Result<StatusResponse> {
        self.client.status().await
    }
    async fn list_images(&self) -> Result<ImageListResponse> {
        self.client.list_images().await
    }
    async fn get_info(&self, id: Id) -> Result<GetInfoResponse> {
        self.client.get_info(id).await
    }

    async fn get_network_ip(&self, id: String) -> Result<IpAddr> {
        // 多次尝试获取网络信息，直到成功为止
        let max_network_attempts = 30; // 最大尝试次数
        let network_wait_interval = std::time::Duration::from_secs(2); // 每次等待2秒
        for network_attempt in 1..=max_network_attempts {
            match self
                .guest_client
                .network_info(GuestId { id: id.clone() })
                .await
            {
                Ok(net_info) => {
                    tracing::info!(
                        "Successfully got network info after {} attempts",
                        network_attempt
                    );
                    return Ok(net_info
                        .interfaces
                        .iter()
                        .filter(|s| s.name == "wg0")
                        .next()
                        .and_then(|s| s.addresses.first())
                        .and_then(|s| IpAddr::from_str(&s.address).ok())
                        .ok_or(anyhow::anyhow!("Failed to get cvm address"))?);
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to get network info, attempt {}/{}: {}",
                        network_attempt,
                        max_network_attempts,
                        err
                    );
                    if network_attempt == max_network_attempts {
                        return Err(anyhow::anyhow!(
                            "Failed to get network info after {} attempts",
                            max_network_attempts
                        ));
                    }
                    tokio::time::sleep(network_wait_interval).await;
                }
            }
        }

        Err(anyhow::anyhow!("Timeout waiting for VM to initialize"))
    }

    async fn get_vm_info(&self) -> Result<Vec<VmInfo>> {
        let vms = self
            .status()
            .await?
            .vms
            .into_iter()
            .filter(|vm| vm.status == "running")
            .collect::<Vec<_>>();

        let mut result = Vec::with_capacity(vms.len());
        for vm in vms {
            let ip = self.get_network_ip(vm.id.clone()).await?;
            result.push((vm, ip).try_into()?);
        }
        Ok(result)
    }
}

mod utils {
    use super::Id;
    use anyhow::Result;
    use std::str::FromStr;
    use uuid::Uuid;

    pub fn uuid_to_id(uuid: &Uuid) -> Id {
        Id {
            id: uuid.to_string(),
        }
    }

    pub fn id_to_uuid(id: &Id) -> Result<Uuid> {
        Ok(Uuid::from_str(&id.id).map_err(|e| anyhow::anyhow!("Invalid UUID: {}", e))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_vm_request() {
        let req = VmConfiguration {
            compose_file: AppCompose {
                manifest_version: 2,
                name: "app1433".to_string(),
                runner: "docker-compose".to_string(),
                docker_compose_file: Some("version: '3'\n\nservices:\n  nginx:\n    image: nginx:1.27.0\n    ports:\n      - \"8080:80\"\n    restart: always\n\n".to_string()),
                docker_config: Default::default(),
                public_logs: true,
                public_sysinfo: true,
                kms_enabled: true,
                tproxy_enabled: true,
                local_key_provider_enabled: false,
                key_provider: None,
                allowed_envs: Default::default(),
                no_instance_id: false,
                features: Default::default(),
            },
            ..Default::default()
        };
        req.validate().unwrap();
    }
}
