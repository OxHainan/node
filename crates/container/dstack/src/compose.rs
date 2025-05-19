use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, str::FromStr};

/// Docker Compose 文件结构
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DockerCompose {
    /// Docker Compose 文件版本
    pub version: String,
    /// 服务定义
    pub services: BTreeMap<String, Service>,
    /// 卷定义
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub volumes: BTreeMap<String, Volume>,
    /// 网络定义
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub networks: BTreeMap<String, Network>,
    /// 顶级环境变量
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    /// 顶级配置
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub configs: BTreeMap<String, Config>,
    /// 顶级密钥
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub secrets: BTreeMap<String, Secret>,
}

/// 端口映射
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMapping {
    /// 主机端口
    pub host_port: u16,
    /// 容器端口
    pub container_port: u16,
}

impl PortMapping {
    /// 创建一个新的端口映射
    pub fn new(host_port: impl Into<u16>, container_port: impl Into<u16>) -> Self {
        Self {
            host_port: host_port.into(),
            container_port: container_port.into(),
        }
    }
}

impl FromStr for PortMapping {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid port mapping format: {}", s));
        }
        let host_port = parts[0]
            .parse()
            .map_err(|_| format!("Invalid host port: {}", parts[0]))?;
        let container_port = parts[1]
            .parse()
            .map_err(|_| format!("Invalid container port: {}", parts[1]))?;
        Ok(PortMapping {
            host_port,
            container_port,
        })
    }
}

impl<'de> Deserialize<'de> for PortMapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for PortMapping {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = format!("{}:{}", self.host_port, self.container_port);
        serializer.serialize_str(&s)
    }
}

/// 服务定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Service {
    /// 服务名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    /// 容器镜像
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// 端口映射
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<PortMapping>,
    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tty: Option<bool>,
    /// 环境变量
    #[serde(default)]
    pub environment: Environment,
    /// 环境变量文件
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_file: Vec<String>,
    /// 卷映射
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<String>,
    /// 依赖服务
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// 构建配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildConfig>,
    /// 命令
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// 入口点
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    /// 用户
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// 主机名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// 网络模式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<String>,
    /// 网络
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<String>,
    /// 健康检查
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<HealthCheck>,
    /// 部署配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deploy: Option<Deploy>,
    /// 容器标签
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    /// 暴露端口
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expose: Vec<String>,
    /// DNS 服务器
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,
    /// DNS 搜索域
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns_search: Vec<String>,
    /// 额外主机
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_hosts: BTreeMap<String, String>,
    /// 日志配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    /// 安全选项
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_opt: Vec<String>,
    /// 系统限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sysctls: Option<Sysctls>,
    /// 配置
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configs: Vec<String>,
    /// 密钥
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
}

/// 构建配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct BuildConfig {
    /// 构建上下文
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Dockerfile 路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile: Option<String>,
    /// 构建参数
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// 构建标签
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    /// 目标阶段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 缓存来源
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_from: Option<String>,
    /// 网络
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
}

/// 健康检查
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct HealthCheck {
    /// 测试命令
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test: Vec<String>,
    /// 间隔时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// 超时时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    /// 重试次数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
    /// 启动时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_period: Option<String>,
}

/// 部署配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Deploy {
    /// 副本数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,
    /// 资源限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
    /// 放置约束
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placement: Vec<String>,
}

/// 资源限制
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Resources {
    /// 资源限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<ResourceSpec>,
    /// 资源预留
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservations: Option<ResourceSpec>,
}

/// 资源规格
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ResourceSpec {
    /// CPU 限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,
    /// 内存限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// 重启策略
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct RestartPolicy {
    /// 条件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// 延迟
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<String>,
    /// 最大尝试次数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
    /// 窗口
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
}

/// 日志配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Logging {
    /// 驱动
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// 选项
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

/// 系统限制
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sysctls {
    /// 映射形式
    Map(BTreeMap<String, String>),
    /// 列表形式
    List(Vec<String>),
}

impl Default for Sysctls {
    fn default() -> Self {
        Sysctls::Map(BTreeMap::new())
    }
}

/// 环境变量，支持映射格式和数组格式
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Environment {
    /// 映射形式，如 KEY: VALUE
    Map(BTreeMap<String, String>),
    /// 列表形式，如 - KEY=VALUE
    List(Vec<String>),
}

impl From<BTreeMap<String, String>> for Environment {
    fn from(map: BTreeMap<String, String>) -> Self {
        Self::Map(map)
    }
}

impl From<Vec<String>> for Environment {
    fn from(list: Vec<String>) -> Self {
        Self::List(list)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::Map(BTreeMap::new())
    }
}

/// 卷定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Volume {
    /// 驱动
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// 驱动选项
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub driver_opts: BTreeMap<String, String>,
    /// 外部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    /// 标签
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    /// 名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// 网络定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Network {
    /// 驱动
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// 驱动选项
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub driver_opts: BTreeMap<String, String>,
    /// 启用 IPv6
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_ipv6: Option<bool>,
    /// 外部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    /// 标签
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    /// 名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 内部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal: Option<bool>,
    /// 附加
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachable: Option<bool>,
}

/// 配置定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// 文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// 外部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    /// 名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// 密钥定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Secret {
    /// 文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// 外部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    /// 名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Default for DockerCompose {
    fn default() -> DockerCompose {
        DockerCompose {
            version: "3".to_owned(),
            services: Default::default(),
            volumes: Default::default(),
            networks: Default::default(),
            environment: Default::default(),
            configs: Default::default(),
            secrets: Default::default(),
        }
    }
}

impl FromStr for DockerCompose {
    type Err = serde_yaml::Error;

    fn from_str(s: &str) -> Result<DockerCompose, Self::Err> {
        serde_yaml::from_str(s)
    }
}

impl Service {
    /// 内联环境变量文件内容到环境变量映射中
    pub fn inline_all(&mut self, base: &Path) -> Result<()> {
        // 处理环境变量文件
        if !self.env_file.is_empty() {
            let mut env_vars = BTreeMap::new();

            for env_file in &self.env_file {
                let file_path = base.join(env_file);
                let content = fs::read_to_string(&file_path)
                    .with_context(|| format!("读取环境变量文件失败: {}", file_path.display()))?;

                // 解析环境变量文件内容 (简单格式: KEY=VALUE)
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue; // 跳过空行和注释
                    }

                    if let Some(pos) = line.find('=') {
                        let key = line[..pos].trim().to_string();
                        let value = line[pos + 1..].trim().to_string();
                        env_vars.insert(key, value);
                    }
                }
            }

            // 合并到现有环境变量中，已存在的不覆盖
            match &mut self.environment {
                Environment::Map(map) => {
                    for (key, value) in env_vars {
                        map.entry(key).or_insert(value);
                    }
                }
                Environment::List(list) => {
                    // 将列表转换为映射
                    let mut map = BTreeMap::new();

                    // 先处理现有的列表项
                    for item in list.drain(..) {
                        if let Some(pos) = item.find('=') {
                            let key = item[..pos].trim().to_string();
                            let value = item[pos + 1..].trim().to_string();
                            map.insert(key, value);
                        }
                    }

                    // 合并环境变量文件中的变量
                    for (key, value) in env_vars {
                        map.entry(key).or_insert(value);
                    }

                    // 替换为映射类型
                    self.environment = Environment::Map(map);
                }
            }

            // 清空环境变量文件列表，因为已经内联到环境变量中
            self.env_file.clear();
        }

        Ok(())
    }

    /// 创建一个简单的服务配置
    pub fn new_simple(image: &str) -> Self {
        let mut service = Service::default();
        service.image = Some(image.to_string());
        service
    }
}

impl DockerCompose {
    /// 从YAML字符串解析DockerCompose
    pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// 将DockerCompose转换为YAML字符串
    pub fn to_yaml_string(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// 从文件读取DockerCompose
    pub fn read_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("读取文件失败: {}", path.as_ref().display()))?;
        Self::from_yaml_str(&content).map_err(|e| e.into())
    }

    /// 从文件读取DockerCompose (别名方法)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::read_from_path(path)
    }

    /// 将DockerCompose写入文件
    pub fn write_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = self.to_yaml_string()?;
        fs::write(path.as_ref(), yaml)
            .with_context(|| format!("写入文件失败: {}", path.as_ref().display()))
    }

    /// 将DockerCompose写入文件 (别名方法)
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.write_to_path(path)
    }

    /// 从JSON字符串解析DockerCompose
    pub fn from_json_str(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 将DockerCompose转换为JSON字符串
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 内联所有外部资源
    pub fn inline_all(&mut self, base: &Path) -> Result<()> {
        for service in self.services.values_mut() {
            service.inline_all(base)?;
        }
        Ok(())
    }

    /// 插值所有环境变量
    pub fn interpolate_all(&mut self) -> Result<()> {
        // 实现环境变量插值功能
        // 这里简单实现，实际可能需要更复杂的逻辑
        for service in self.services.values_mut() {
            // 使用顶级环境变量填充服务环境变量中的占位符
            match &mut service.environment {
                Environment::Map(map) => {
                    for (env_key, env_value) in &self.environment {
                        for (_, service_value) in map.iter_mut() {
                            // 替换 ${VAR} 或 $VAR 形式的环境变量
                            if service_value.contains(&format!("${{{}}}", env_key)) {
                                *service_value =
                                    service_value.replace(&format!("${{{}}}", env_key), env_value);
                            } else if service_value.contains(&format!("${}", env_key)) {
                                *service_value =
                                    service_value.replace(&format!("${}", env_key), env_value);
                            }
                        }
                    }
                }
                Environment::List(list) => {
                    // 处理列表形式的环境变量
                    for item in list.iter_mut() {
                        for (env_key, env_value) in &self.environment {
                            // 替换 ${VAR} 或 $VAR 形式的环境变量
                            if item.contains(&format!("${{{}}}", env_key)) {
                                *item = item.replace(&format!("${{{}}}", env_key), env_value);
                            } else if item.contains(&format!("${}", env_key)) {
                                *item = item.replace(&format!("${}", env_key), env_value);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 转换为独立文件
    pub fn make_standalone(&mut self, base: &Path) -> Result<()> {
        self.interpolate_all()?;
        self.inline_all(base)
    }

    /// 创建一个新的 Docker Compose 文件
    pub fn new(version: &str) -> Self {
        DockerCompose {
            version: version.to_string(),
            services: BTreeMap::new(),
            volumes: BTreeMap::new(),
            networks: BTreeMap::new(),
            environment: BTreeMap::new(),
            configs: BTreeMap::new(),
            secrets: BTreeMap::new(),
        }
    }

    /// 添加服务
    pub fn add_service(&mut self, name: &str, service: Service) -> &mut Self {
        self.services.insert(name.to_string(), service);
        self
    }

    /// 添加卷
    pub fn add_volume(&mut self, name: &str, volume: Volume) -> &mut Self {
        self.volumes.insert(name.to_string(), volume);
        self
    }

    /// 添加网络
    pub fn add_network(&mut self, name: &str, network: Network) -> &mut Self {
        self.networks.insert(name.to_string(), network);
        self
    }

    /// 添加环境变量
    pub fn add_env(&mut self, key: &str, value: &str) -> &mut Self {
        self.environment.insert(key.to_string(), value.to_string());
        self
    }
}
