use anyhow::Result;
use dstack::{compose::DockerCompose, types::CreateVmRequest, PodClient, PodClientT, TappdClientT};
use tappd_rpc::{DeriveKeyArgs, TdxQuoteArgs};

#[tokio::main]
pub async fn main() -> Result<()> {
    let client = PodClient::new("http://127.0.0.1:33001", "http://127.0.0.1:8020").await?;
    // let docker_compose = DockerCompose::from_file("docker-compose.yaml")?;
    // let vm = client
    //     .create_vm(CreateVmRequest {
    //         name: "app143".to_string(),
    //         docker_compose: docker_compose.to_yaml_string()?,
    //         ..Default::default()
    //     })
    //     .await?;
    // println!("{}", serde_json::to_string_pretty(&vm)?);
    // let vms = client.get_vm_info().await?;
    // for vm in vms {
    //     println!("{}", vm.base_url());
    // }
    let info = client
        .tdx_quote(TdxQuoteArgs {
            report_data: vec![0x1, 0x2],
            hash_algorithm: "sha512".to_string(),
            prefix: "app-data:".to_string(),
        })
        .await?;
    println!("{:?}", info);
    let info = client.info().await?;
    println!("{:?}", info);
    Ok(())
}
