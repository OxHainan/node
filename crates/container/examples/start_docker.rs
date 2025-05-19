use mp_container::{
    docker::DockerContainerEnvironment, ContainerEnvironment, CreateVmRequest, DockerCompose,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let docker_file =
        DockerCompose::from_yaml_str(include_str!("../dstack/examples/docker-compose.yaml"))?;

    let docker = DockerContainerEnvironment::default();
    docker.init_vms().await?;
    let res = docker.get_vms().await?;
    println!("{:?}", res);
    let vm_info = docker
        .create_container(CreateVmRequest {
            name: "web2_style".to_string(),
            docker_compose: docker_file.to_yaml_string()?,
            ..Default::default()
        })
        .await?;
    println!("{:?}", vm_info);
    Ok(())
}
