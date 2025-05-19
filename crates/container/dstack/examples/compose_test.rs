use anyhow::Result;
use dstack::compose::{DockerCompose, Network, PortMapping, Service, Volume};
use std::collections::BTreeMap;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    // println!("===== Docker Compose 文件解析测试 =====");

    // // 测试1: 创建一个新的 Docker Compose 文件
    // create_compose_file()?;

    // // 测试2: 解析现有的 Docker Compose 文件
    // parse_compose_file()?;

    // // 测试3: 环境变量插值测试
    // env_variable_interpolation()?;

    // // 测试4: 从 JSON 创建 Compose 文件
    // from_json_to_yaml()?;

    // 测试5: 文件读写测试
    file_io_test()?;

    println!("\n所有测试完成!");
    Ok(())
}

// 测试1: 创建一个新的 Docker Compose 文件
fn create_compose_file() -> Result<()> {
    println!("\n----- 测试1: 创建 Docker Compose 文件 -----");

    // 创建一个新的 Docker Compose 文件
    let mut compose = DockerCompose::new("3.8");

    // 添加 Nginx 服务
    let mut nginx = Service::new_simple("nginx:latest");
    nginx.container_name = Some("nginx".to_string());
    nginx.ports = vec![
        PortMapping::new(8080u16, 80u16),
        PortMapping::new(443u16, 443u16),
    ];
    nginx.restart = Some("always".to_string());
    nginx.environment = dstack::compose::Environment::Map(BTreeMap::from([
        ("NGINX_HOST".to_string(), "example.com".to_string()),
        ("NGINX_PORT".to_string(), "80".to_string()),
    ]));

    // 添加 Redis 服务
    let mut redis = Service::new_simple("redis:alpine");
    redis.container_name = Some("redis".to_string());
    redis.ports = vec![PortMapping::new(6379u16, 6379u16)];
    redis.restart = Some("unless-stopped".to_string());
    redis.volumes = vec!["redis-data:/data".to_string()];

    // 添加 PostgreSQL 服务
    let mut postgres = Service::new_simple("postgres:13");
    postgres.container_name = Some("postgres".to_string());
    postgres.ports = vec![PortMapping::new(5432u16, 5432u16)];
    postgres.environment = dstack::compose::Environment::Map(BTreeMap::from([
        ("POSTGRES_USER".to_string(), "user".to_string()),
        ("POSTGRES_PASSWORD".to_string(), "password".to_string()),
        ("POSTGRES_DB".to_string(), "mydb".to_string()),
    ]));
    postgres.volumes = vec!["postgres-data:/var/lib/postgresql/data".to_string()];

    // 将服务添加到 DockerCompose
    compose
        .add_service("web", nginx)
        .add_service("cache", redis)
        .add_service("db", postgres);

    // 添加卷
    compose
        .add_volume("redis-data", Volume::default())
        .add_volume("postgres-data", Volume::default());

    // 添加网络
    let mut app_network = Network::default();
    app_network.driver = Some("bridge".to_string());
    compose.add_network("app-network", app_network);

    // 将 DockerCompose 转换为 YAML 字符串
    let yaml = compose.to_yaml_string()?;
    println!("生成的 Docker Compose 文件:\n{}", yaml);

    Ok(())
}

// 测试2: 解析现有的 Docker Compose 文件
fn parse_compose_file() -> Result<()> {
    println!("\n----- 测试2: 解析 Docker Compose 文件 -----");

    let yaml = r#"
version: '3'
services:
  web:
    image: nginx:latest
    ports:
      - "80:80"
    volumes:
      - ./html:/usr/share/nginx/html
    depends_on:
      - api
  api:
    image: node:14
    working_dir: /app
    volumes:
      - ./api:/app
    command: npm start
    environment:
      NODE_ENV: production
      PORT: 3000
volumes:
  data:
    driver: local
networks:
  frontend:
    driver: bridge
  backend:
    driver: bridge
"#;

    let compose = DockerCompose::from_yaml_str(yaml)?;

    println!("解析结果:");
    println!("版本: {}", compose.version);
    println!("服务数量: {}", compose.services.len());
    println!("卷数量: {}", compose.volumes.len());
    println!("网络数量: {}", compose.networks.len());

    // 检查服务详情
    if let Some(web_service) = compose.services.get("web") {
        println!("\nWeb 服务详情:");
        println!("  镜像: {:?}", web_service.image);
        println!("  端口: {:?}", web_service.ports);
        println!("  依赖: {:?}", web_service.depends_on);
    }

    if let Some(api_service) = compose.services.get("api") {
        println!("\nAPI 服务详情:");
        println!("  镜像: {:?}", api_service.image);
        println!("  工作目录: {:?}", api_service.working_dir);
        println!("  命令: {:?}", api_service.command);
        println!("  环境变量: {:?}", api_service.environment);
    }

    Ok(())
}

// 测试3: 环境变量插值测试
fn env_variable_interpolation() -> Result<()> {
    println!("\n----- 测试3: 环境变量插值 -----");

    let mut compose = DockerCompose::new("3");

    // 添加顶级环境变量
    compose
        .add_env("APP_PORT", "3000")
        .add_env("DB_VERSION", "13")
        .add_env("CACHE_VERSION", "alpine");

    // 添加使用环境变量的服务
    let mut app = Service::new_simple("node:14");
    app.ports = vec![PortMapping::new(3000u16, 3000u16)];

    let mut db = Service::new_simple("postgres:${DB_VERSION}");
    db.environment = dstack::compose::Environment::Map(BTreeMap::from([
        ("POSTGRES_DB".to_string(), "myapp".to_string()),
        ("POSTGRES_PORT".to_string(), "3000".to_string()),
    ]));

    let cache = Service::new_simple("redis:${CACHE_VERSION}");

    compose
        .add_service("app", app)
        .add_service("db", db)
        .add_service("cache", cache);

    // 输出插值前
    println!("插值前的 Docker Compose 文件:");
    println!("{}", compose.to_yaml_string()?);

    // 插值环境变量
    compose.interpolate_all()?;

    // 输出插值后
    println!("\n插值后的 Docker Compose 文件:");
    println!("{}", compose.to_yaml_string()?);

    Ok(())
}

// 测试4: 从 JSON 创建 Compose 文件
fn from_json_to_yaml() -> Result<()> {
    println!("\n----- 测试4: JSON 与 YAML 转换 -----");

    let json = r#"{
        "version": "3.7",
        "services": {
            "app": {
                "image": "node:14",
                "ports": ["3000:3000"],
                "environment": {
                    "NODE_ENV": "production"
                }
            },
            "db": {
                "image": "mongo:4",
                "volumes": ["mongo-data:/data/db"]
            }
        },
        "volumes": {
            "mongo-data": {}
        }
    }"#;

    // 从 JSON 创建 DockerCompose
    let compose = DockerCompose::from_json_str(json)?;

    println!("从 JSON 创建的 Docker Compose 文件 (YAML 格式):");
    println!("{}", compose.to_yaml_string()?);

    // 转回 JSON
    let json_again = compose.to_json_string()?;
    println!("\n转回 JSON 格式:");
    println!("{}", json_again);

    Ok(())
}

// 测试5: 文件读写测试
fn file_io_test() -> Result<()> {
    println!("\n----- 测试5: 文件读写测试 -----");

    let test_file = include_str!("docker-compose.yaml");
    println!("\n从文件读取的 Compose 文件:");
    println!("{}", test_file);

    // // 创建一个简单的 Compose 文件
    // let mut compose = DockerCompose::new("3");

    // let mut app = Service::new_simple("myapp:latest");
    // app.ports = vec!["8080:8080".to_string()];

    // compose.add_service("app", app);

    // // 写入文件
    // compose.write_to_file(test_file)?;
    // println!("已将 Compose 文件写入: {}", test_file);

    // 从文件读取
    let read_compose = DockerCompose::from_yaml_str(test_file)?;
    println!("从文件读取的 Compose 文件:");
    println!("{:?}", read_compose.services);
    println!("{}", serde_json::to_string_pretty(&read_compose)?);
    println!("{}", read_compose.to_yaml_string()?);

    // // 清理测试文件
    // std::fs::remove_file(test_file)?;
    println!("已删除测试文件: {}", test_file);

    Ok(())
}
