mod utils;

use anyhow::anyhow;
use anyhow::Result;
use bollard::errors::Error;
use bollard::image::CreateImageOptions;
use bollard::models::{
    ContainerCreateResponse, ContainerInspectResponse, CreateImageInfo, ImageInspect,
};
use bollard::{models::ContainerSummary, secret::HostConfig, Docker};
use dstack::types::AccessControl;
use dstack::types::AgentConfiguration;
use dstack::types::AuthorizationType;
use dstack::types::CreateAction;
use dstack::types::PricingModel;
use dstack::DeriveK256KeyArgs;
use dstack::DeriveK256KeyResponse;
use dstack::DeriveKeyArgs;
use dstack::DeriveKeyResponse;
use dstack::RawQuoteArgs;
use dstack::TdxQuoteArgs;
use dstack::TdxQuoteResponse;
use dstack::WorkerInfo;
use dstack::{TappdClient, TappdClientT};
use futures::TryStreamExt;
use mp_common::utils::uuid_to_h128;
use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions,
    RestartContainerOptions, StartContainerOptions,
};
use dstack::compose::DockerCompose;
use utils::{parse_container_inspect_response_port, port_bindings, tcp_port};

use crate::config::default_tappd_host;
use crate::utils::string_to_uuid;
use crate::ContainerDetail;
use crate::{ContainerEnvironment, ContainerInfo, ContainerStatus};

pub fn from_config(config: DockerCompose) -> Vec<(String, Config<String>)> {
    let mut configs = Vec::new();
    for (name, service) in config.services {
        let ports = service.ports.first().unwrap();
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(tcp_port(&ports.container_port.to_string()), HashMap::new());

        let config = Config {
            image: service.image,
            exposed_ports: Some(exposed_ports),
            host_config: Some(HostConfig {
                port_bindings: port_bindings(ports.host_port, ports.container_port),
                ..Default::default()
            }),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        configs.push((name, config));
    }
    configs
}

#[derive(Clone, Debug)]
pub struct VmInfo {
    pub id: Uuid,
    pub name: String,
    pub docker_name: String,
    pub address: SocketAddr,
    pub status: ContainerStatus,
    pub instance_id: String,
}

pub struct DockerContainerEnvironment {
    docker: Arc<Docker>,
    containers: Arc<Mutex<HashMap<Uuid, ContainerDetail>>>,
    tappd_client: Arc<Mutex<TappdClient>>,
}

impl Debug for DockerContainerEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DockerContainerEnvironment")
    }
}

impl Default for DockerContainerEnvironment {
    fn default() -> Self {
        Self::new(default_tappd_host())
    }
}

impl DockerContainerEnvironment {
    pub fn get_tappd_client(&self) -> Arc<Mutex<dyn TappdClientT>> {
        self.tappd_client.clone()
    }

    pub fn new(base_url: impl Into<String>) -> Self {
        // First try to get Docker connection information from environment variables
        let docker = if let Ok(docker) = Docker::connect_with_local_defaults() {
            docker
        } else {
            // If default connection fails, try to get the current active context from docker context
            info!("Failed to connect with local defaults, trying to detect Docker context");

            // Use Docker CLI to get current context information
            let output = std::process::Command::new("docker")
                .args(["context", "inspect"])
                .output()
                .ok();

            if let Some(output) = output {
                if output.status.success() {
                    // Parse JSON output to get Docker endpoint
                    if let Ok(context_info) =
                        serde_json::from_slice::<serde_json::Value>(&output.stdout)
                    {
                        if let Some(endpoint) =
                            context_info[0]["Endpoints"]["docker"]["Host"].as_str()
                        {
                            info!("Found Docker endpoint from context: {}", endpoint);
                            // Try to connect using the found endpoint
                            if endpoint.starts_with("unix://") {
                                let socket_path = &endpoint[7..]; // Remove "unix://" prefix
                                if let Ok(docker) = Docker::connect_with_socket(
                                    socket_path,
                                    120,
                                    bollard::API_DEFAULT_VERSION,
                                ) {
                                    return Self {
                                        docker: Arc::new(docker),
                                        containers: Arc::new(Mutex::new(HashMap::new())),
                                        tappd_client: Arc::new(Mutex::new(TappdClient::new(
                                            base_url,
                                        ))),
                                    };
                                }
                            }
                        }
                    }
                }
            }

            // If all attempts fail, throw an error
            panic!("Could not connect to Docker. Please ensure Docker is running and properly configured.")
        };

        Self {
            docker: Arc::new(docker),
            containers: Arc::new(Mutex::new(HashMap::new())),
            tappd_client: Arc::new(Mutex::new(TappdClient::new(base_url))),
        }
    }

    /// Generate a container name for a module
    fn generate_container_name(module_id: &str) -> String {
        format!("mp-{}", module_id)
    }

    // pub async fn get_vms(&self) -> Result<Vec<VmInfo>, anyhow::Error> {
    //     let containers = self.containers.lock().await;
    //     let vms = containers.values().cloned().collect();
    //     Ok(vms)
    // }

    // pub async fn init_vms(&self) -> Result<(), anyhow::Error> {
    //     let containers = self.list_containers().await?;

    //     let mut con = self.containers.lock().await;
    //     for container in containers {
    //         if let Some(names) = &container.names {
    //             let name = names.iter().find(|name| name.starts_with("/mp-"));
    //             let Some(docker_name) = name else {
    //                 continue;
    //             };

    //             let name_id = docker_name
    //                 .strip_prefix("/mp-")
    //                 .ok_or_else(|| anyhow!("Invalid container name format: {}", docker_name))?;
    //             let id = string_to_uuid(Some(name_id.to_string()));
    //             let status = if let Some(state) = container.state {
    //                 match state.to_lowercase().as_str() {
    //                     "running" => ContainerStatus::Running,
    //                     "exited" => ContainerStatus::Stopped,
    //                     "created" => ContainerStatus::Starting,
    //                     _ => ContainerStatus::Error("Unknown container status".to_string()),
    //                 }
    //             } else {
    //                 ContainerStatus::Error("Unknown container status".to_string())
    //             };

    //             let public_port = container
    //                 .ports
    //                 .as_ref()
    //                 .and_then(|ports| ports.first())
    //                 .and_then(|port| port.public_port)
    //                 .unwrap_or(0);

    //             let vm_info = VmInfo {
    //                 id,
    //                 name: name_id.to_string(),
    //                 docker_name: docker_name
    //                     .strip_prefix("/")
    //                     .ok_or_else(|| anyhow!("Invalid container name format: {}", docker_name))?
    //                     .to_string(),
    //                 address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), public_port),
    //                 status,
    //                 instance_id: id.to_string(),
    //             };
    //             con.insert(id, vm_info);
    //         }
    //     }
    //     Ok(())
    // }

    pub async fn list_containers(&self) -> Result<Vec<ContainerSummary>, Error> {
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await?;
        info!("[DOCKER] Found {} containers", containers.len());

        Ok(containers
            .into_iter()
            .filter(|c| {
                if let Some(names) = &c.names {
                    names.iter().any(|name| name.starts_with("/mp-"))
                } else {
                    false
                }
            })
            .collect())
    }

    async fn create_image(
        &self,
        images: String,
        tag: String,
    ) -> anyhow::Result<Vec<CreateImageInfo>> {
        info!("[DOCKER] Starting download of image {}:{}", images, tag);

        let image_stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: images.clone(),
                tag: tag.clone(),
                ..Default::default()
            }),
            None,
            None,
        );

        // Process the stream to provide progress updates
        let mut progress_reported = false;
        let mut result = Vec::new();

        tokio::pin!(image_stream);

        while let Some(info) = image_stream.try_next().await? {
            result.push(info.clone());

            // Log progress information
            if let Some(status) = info.status {
                if status.contains("Pulling from") {
                    info!("[DOCKER] Pulling image {}:{} from registry", images, tag);
                } else if status.contains("Downloading") || status.contains("Extracting") {
                    if !progress_reported {
                        info!(
                            "[DOCKER] Downloading and extracting image layers for {}:{}",
                            images, tag
                        );
                        progress_reported = true;
                    }
                } else if status.contains("Downloaded newer image")
                    || status.contains("up to date")
                    || status.contains("Pull complete")
                {
                    info!("[DOCKER] {}", status);
                }
            }
        }

        info!("[DOCKER] Successfully prepared image {}:{}", images, tag);
        Ok(result)
    }

    async fn create_container(
        &self,
        name: String,
        config: Config<String>,
    ) -> anyhow::Result<ContainerInfo> {
        let mut containers = self.containers.lock().await;
        let vm_id = string_to_uuid(Some(name.clone()));
        if let Some(info) = containers.get_mut(&vm_id) {
            info!("[DOCKER] Container {} already exists", name);
            let container_name = Self::generate_container_name(&info.info.name);
            if info.info.status == ContainerStatus::Stopped {
                self.docker
                    .restart_container(&container_name, None::<RestartContainerOptions>)
                    .await?;

                info!("[DOCKER] Container {} restarted successfully", name);
                let inspect_container = self
                    .docker
                    .inspect_container(&container_name, None::<InspectContainerOptions>)
                    .await?;

                info.info.status = ContainerStatus::Running;
                info.info.address = SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    parse_container_inspect_response_port(&inspect_container),
                );
            }

            return Ok(ContainerInfo {
                contract_id: uuid_to_h128(&vm_id),
                address: info.info.address,
                name: info.info.name.clone(),
                status: info.info.status.clone(),
                id: vm_id,
                instance_id: info.info.instance_id.clone(),
            });
        }

        let Some(image_name) = config.image.clone() else {
            return Err(anyhow!("Image name is required"));
        };

        // Check if the image exists locally
        info!("[DOCKER] Checking if image {} exists locally", image_name);
        let image_exists = self.inspect_image(&image_name).await.is_ok();

        if !image_exists {
            info!(
                "[DOCKER] Image {} not found locally, will download from registry",
                image_name
            );
            // Parse image name to get tag if specified
            let (image_base, tag) = if image_name.contains(":") {
                let parts: Vec<&str> = image_name.split(":").collect();
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (image_name.clone(), "latest".to_string())
            };

            self.create_image(image_base, tag).await?;
            info!(
                "[DOCKER] Image {} successfully downloaded and ready to use",
                image_name
            );
        } else {
            info!("[DOCKER] Using existing local image: {}", image_name);
        }

        let container_name = Self::generate_container_name(&name);

        // Get IP address of the container
        let Some(host_config) = config.host_config.clone() else {
            return Err(anyhow!("Host config is required"));
        };

        let Some(ports) = host_config.port_bindings else {
            return Err(anyhow!("Port bindings is required"));
        };

        let Some(host_port) = ports
            .iter()
            .filter_map(|(_, host_ip)| {
                host_ip
                    .as_ref()
                    .and_then(|ips| ips.iter().filter_map(|ip| ip.host_port.clone()).next())
            })
            .next()
        else {
            return Err(anyhow!("IP address is required"));
        };

        let host_port: u16 = host_port
            .parse()
            .map_err(|_| anyhow!("Invalid port number"))?;

        // Create container
        self.create_container_inner(container_name.clone(), config)
            .await?;

        // Start container
        self.docker
            .start_container(&container_name, None::<StartContainerOptions<String>>)
            .await?;
        let vm_info = VmInfo {
            id: vm_id,
            name: name.clone(),
            docker_name: container_name,
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), host_port),
            status: ContainerStatus::Running,
            instance_id: vm_id.to_string(),
        };
        // Store container info

        Ok(ContainerInfo {
            contract_id: uuid_to_h128(&vm_id),
            name: vm_info.name,
            address: vm_info.address,
            status: vm_info.status,
            id: vm_id,
            instance_id: vm_id.to_string(),
        })
    }

    async fn inspect_image(&self, image_name: &str) -> anyhow::Result<ImageInspect> {
        self.docker
            .inspect_image(image_name)
            .await
            .map_err(Into::into)
    }

    async fn create_container_inner(
        &self,
        name: String,
        config: Config<String>,
    ) -> anyhow::Result<ContainerCreateResponse> {
        self.docker
            .create_container(
                Some(CreateContainerOptions {
                    name,
                    ..Default::default()
                }),
                config,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn inspect_container(
        &self,
        container_name: &str,
    ) -> anyhow::Result<ContainerInspectResponse> {
        self.docker
            .inspect_container(
                container_name,
                Some(InspectContainerOptions { size: false }),
            )
            .await
            .map_err(Into::into)
    }

    async fn stop_container(&self, vm_id: &Uuid) -> anyhow::Result<()> {
        let mut containers = self.containers.lock().await;
        let vm_info = containers
            .get_mut(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        if vm_info.info.status != ContainerStatus::Running {
            return Ok(());
        }
        let container_name = Self::generate_container_name(&vm_info.info.name);

        self.docker
            .stop_container(&container_name, None)
            .await?;
        vm_info.info.status = ContainerStatus::Stopped;
        info!("[DOCKER] Container {} stopped successfully", vm_id);
        Ok(())
    }

    pub async fn get_container_status(&self, vm_id: &Uuid) -> anyhow::Result<ContainerStatus> {
        let containers = self.containers.lock().await;
        let vm_info = containers
            .get(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        Ok(vm_info.info.status.clone())
    }

    async fn get_running_containers(&self) -> Result<Vec<ContainerDetail>, anyhow::Error> {
        let containers = self.containers.lock().await;
        let running_containers = containers
            .iter()
            .filter(|(_, info)| info.info.status == ContainerStatus::Running)
            .map(|(_, info)| info.clone())
            .collect();
        Ok(running_containers)
    }

    async fn start_container(&self, vm_id: &Uuid) -> Result<ContainerInfo, anyhow::Error> {
        let mut containers = self.containers.lock().await;
        let vm_info = containers
            .get_mut(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        let container_name = Self::generate_container_name(&vm_info.info.name);
        if vm_info.info.status != ContainerStatus::Running {
            self.docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await?;
            vm_info.info.status = ContainerStatus::Running;
            info!("[DOCKER] Container {} started successfully", vm_id);
            let inspect_container = self
                .docker
                .inspect_container(&container_name, None::<InspectContainerOptions>)
                .await?;
            vm_info.info.address = SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                parse_container_inspect_response_port(&inspect_container),
            );
        }

        Ok(ContainerInfo {
            contract_id: vm_info.info.contract_id,
            address: vm_info.info.address,
            status: vm_info.info.status.clone(),
            instance_id: vm_info.info.instance_id.clone(),
            name: vm_info.info.name.clone(),
            id: vm_info.info.id,
        })
    }
}

#[async_trait::async_trait]
impl ContainerEnvironment for DockerContainerEnvironment {
    async fn get_running_containers(&self) -> anyhow::Result<Vec<ContainerDetail>> {
        self.get_running_containers().await
    }

    async fn remove_container(&self, vm_id: &Uuid) -> anyhow::Result<()> {
        let mut containers = self.containers.lock().await;
        let mut vm_info = containers
            .remove(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        let container_name = Self::generate_container_name(&vm_info.info.name);
        if vm_info.info.status == ContainerStatus::Running {
    
            self.docker
                .stop_container(&container_name, None)
                .await?;
            vm_info.info.status = ContainerStatus::Stopping;
        }

        if vm_info.info.status == ContainerStatus::Stopping {
            self.docker
                .remove_container(&container_name, None)
                .await?;
        }

        Ok(())
    }

    async fn create_container(&self, req: AgentConfiguration) -> anyhow::Result<ContainerInfo> {
        let docker_compose = DockerCompose::from_yaml_str(&req.docker_compose)?;
        let configs = from_config(docker_compose);
        let mut containers = Vec::new();
        if configs.is_empty() {
            return Err(anyhow!("No service defined in docker compose"));
        }

        // 检查容器是否已经存在
        let container_name = Self::generate_container_name(&req.name);
        {
            let containers = self.containers.lock().await;
            if let Some(info) = containers.get(&string_to_uuid(Some(container_name.clone()))) {
                return Ok(info.info.clone());
            }
        }
        for (name, config) in configs {
            containers.push(self.create_container(name.clone(), config).await?);
        }

        let vm_info = containers[0].clone();

        let info = ContainerDetail {
            agent_name: req.name.clone(),
            description: "docker".to_string(),
            tags: vec![],
            pricing: PricingModel::Free,
            daily_call_quote: 100,
            access: AccessControl::Public,
            authorization_type: AuthorizationType::APIKEY,
            action: CreateAction::Agent(req.clone()),
            info: vm_info.clone(),
        };

        self.containers.lock().await.insert(
            vm_info.id,
            info
        );

        Ok(containers[0].clone())
    }

    async fn get_container(&self, vm_id: &Uuid) -> anyhow::Result<ContainerDetail> {
        let containers = self.containers.lock().await;
        let vm_info = containers
            .get(vm_id)
            .ok_or(anyhow!("Contract {:?} not found", uuid_to_h128(vm_id)))?;
        Ok(vm_info.clone())
    }

    async fn start_container(&self, vm_id: &Uuid) -> anyhow::Result<ContainerInfo> {
        self.start_container(vm_id).await
    }

    async fn stop_container(&self, vm_id: &Uuid) -> anyhow::Result<()> {
        self.stop_container(vm_id).await
    }

    async fn get_container_status(&self, vm_id: &Uuid) -> anyhow::Result<ContainerStatus> {
        self.get_container_status(vm_id).await
    }
}

#[async_trait::async_trait]
impl TappdClientT for DockerContainerEnvironment {
    async fn derive_key(&self, args: DeriveKeyArgs) -> Result<DeriveKeyResponse> {
        self.tappd_client.lock().await.derive_key(args).await
    }
    async fn derive_k256_key(&self, args: DeriveK256KeyArgs) -> Result<DeriveK256KeyResponse> {
        self.tappd_client.lock().await.derive_k256_key(args).await
    }
    async fn tdx_quote(&self, args: TdxQuoteArgs) -> Result<TdxQuoteResponse> {
        self.tappd_client.lock().await.tdx_quote(args).await
    }
    async fn raw_quote(&self, args: RawQuoteArgs) -> Result<TdxQuoteResponse> {
        self.tappd_client.lock().await.raw_quote(args).await
    }
    async fn info(&self) -> Result<WorkerInfo> {
        self.tappd_client.lock().await.info().await
    }
}
