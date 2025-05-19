use anyhow::anyhow;
use anyhow::Result;
use dstack::types::AgentConfiguration;
use dstack::types::CreateAction;
use dstack::DeriveK256KeyArgs;
use dstack::DeriveK256KeyResponse;
use dstack::DeriveKeyArgs;
use dstack::DeriveKeyResponse;
use dstack::RawQuoteArgs;
use dstack::Status;
use dstack::TappdClientT;
use dstack::TdxQuoteArgs;
use dstack::TdxQuoteResponse;
use dstack::WorkerInfo;
use dstack::{PodClient, PodClientT};
use mp_common::utils::uuid_to_h128;
use std::fmt::Debug;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use uuid::Uuid;

use crate::utils::string_to_uuid;
use crate::ContainerDetail;
use crate::ContainerEnvironment;
use crate::ContainerInfo;
use crate::ContainerStatus;

#[derive(Clone)]
pub struct ContainerVirtureManager {
    client: Arc<Mutex<PodClient>>,
    containers: Arc<Mutex<HashMap<Uuid, ContainerDetail>>>,
}

impl Debug for ContainerVirtureManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ContainerVirtureManager")
    }
}

impl ContainerVirtureManager {
    pub async fn new(base_url: impl Into<String>, tappd_url: impl Into<String>) -> Result<Self> {
        let client = Arc::new(Mutex::new(PodClient::new(base_url, tappd_url).await?));

        Ok(Self {
            client,
            containers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn get_tappd_client(&self) -> Arc<Mutex<dyn TappdClientT>> {
        self.client.clone()
    }

    pub async fn create_vm(&self, req: AgentConfiguration) -> Result<ContainerDetail> {
        let model_id = string_to_uuid(Some(req.name.clone()));
        info!("[DOCKER] Creating container for module: {}", req.name);
        let vm_info = self.containers.lock().await.get(&model_id).cloned();
        let client = self.client.lock().await;
        if let Some(info) = vm_info {
            info!("[DOCKER] Container {} already exists", req.name);
            if info.info.status != ContainerStatus::Running {
                client.start_vm(&info.info.id).await?;
                info!("[DOCKER] Container {} started successfully", req.name);
            }
            return Ok(info);
        }

        let vm = client.create_vm(req.clone()).await?;
        let info = ContainerDetail {
            agent_name: req.name.clone(),
            description: Default::default(),
            tags: Default::default(),
            pricing: Default::default(),
            daily_call_quote: 0,
            access: Default::default(),
            authorization_type: Default::default(),
            action: CreateAction::Agent(req.clone()),
            info: ContainerInfo {
                contract_id: uuid_to_h128(&model_id),
                name: req.name.clone(),
                address: vm.base_url(),
                status: ContainerStatus::Running,
                id: model_id,
                instance_id: vm.instance_id.clone(),
            },
        };
        self.containers.lock().await.insert(model_id, info.clone());
        Ok(info)
    }

    pub async fn stop(&self, vm_id: &Uuid) -> Result<()> {
        let client = self.client.lock().await;

        let mut containers = self.containers.lock().await;
        let vm_info = containers
            .get_mut(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        if vm_info.info.status != ContainerStatus::Running {
            return Ok(());
        }
        client.stop_vm(&vm_info.info.id).await?;
        vm_info.info.status = ContainerStatus::Stopped;
        info!("[DOCKER] Container {} stopped successfully", vm_info.info.name);
        Ok(())
    }

    pub async fn get_container_status(&self, vm_id: &Uuid) -> Result<ContainerStatus> {
        let containers = self.containers.lock().await;
        let vm_info = containers
            .get(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        Ok(vm_info.info.status.clone())
    }

    async fn get_running_containers(&self) -> Result<Vec<ContainerDetail>> {
        let containers = self.containers.lock().await;
        let running = containers
            .values()
            .filter(|c| c.info.status == ContainerStatus::Running)
            .map(|c| c.clone())
            .collect();
        Ok(running)
    }
}

impl From<&Status> for ContainerStatus {
    fn from(status: &Status) -> Self {
        match status {
            Status::Running => ContainerStatus::Running,
            Status::Stopped => ContainerStatus::Stopped,
            Status::Stopping => ContainerStatus::Stopping,
            Status::Exited => ContainerStatus::Stopped,
        }
    }
}

impl From<Status> for ContainerStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Running => ContainerStatus::Running,
            Status::Stopped => ContainerStatus::Stopped,
            Status::Stopping => ContainerStatus::Stopping,
            Status::Exited => ContainerStatus::Stopped,
        }
    }
}

#[async_trait::async_trait]
impl ContainerEnvironment for ContainerVirtureManager {
    async fn create_container(&self, req: AgentConfiguration) -> Result<ContainerInfo> {
        self.create_vm(req).await.map(|vm| vm.info)
    }

    async fn remove_container(&self, vm_id: &Uuid) -> Result<()> {
        let mut containers = self.containers.lock().await;
        let mut vm_info = containers
            .remove(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        let client = self.client.lock().await;
        if vm_info.info.status == ContainerStatus::Running {
            client.stop_vm(&vm_info.info.id).await?;
            vm_info.info.status = ContainerStatus::Stopping;
        }

        if vm_info.info.status == ContainerStatus::Stopping {
            client.remove_vm(&vm_info.info.id).await?;
        }

        Ok(())
    }

    async fn get_container(&self, vm_id: &Uuid) -> Result<ContainerDetail> {
        debug!("[DOCKER] Getting container {}", vm_id);
        let containers = self.containers.lock().await;
        let vm_info = containers
            .get(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        Ok(vm_info.clone())
    }

    async fn start_container(&self, vm_id: &Uuid) -> Result<ContainerInfo> {
        debug!("[DOCKER] Starting container {}", vm_id);
        let client = self.client.lock().await;
        let mut containers = self.containers.lock().await;
        let vm_info = containers
            .get_mut(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        if vm_info.info.status != ContainerStatus::Running {
            client.start_vm(&vm_info.info.id).await?;
            vm_info.info.status = ContainerStatus::Running;
            info!("[DOCKER] Container {} started successfully", vm_info.info.name);
        }

        Ok(vm_info.info.clone())
    }

    async fn stop_container(&self, vm_id: &Uuid) -> Result<()> {
        self.stop(vm_id).await
    }

    async fn get_container_status(&self, vm_id: &Uuid) -> Result<ContainerStatus> {
        self.get_container_status(vm_id).await.map(|s| s.into())
    }

    async fn get_running_containers(&self) -> Result<Vec<ContainerDetail>> {
        self.get_running_containers().await
    }
}

#[async_trait::async_trait]
impl TappdClientT for ContainerVirtureManager {
    async fn derive_key(&self, args: DeriveKeyArgs) -> Result<DeriveKeyResponse> {
        self.client.lock().await.derive_key(args).await
    }
    async fn derive_k256_key(&self, args: DeriveK256KeyArgs) -> Result<DeriveK256KeyResponse> {
        self.client.lock().await.derive_k256_key(args).await
    }
    async fn tdx_quote(&self, args: TdxQuoteArgs) -> Result<TdxQuoteResponse> {
        self.client.lock().await.tdx_quote(args).await
    }
    async fn raw_quote(&self, args: RawQuoteArgs) -> Result<TdxQuoteResponse> {
        self.client.lock().await.raw_quote(args).await
    }
    async fn info(&self) -> Result<WorkerInfo> {
        self.client.lock().await.info().await
    }
}
