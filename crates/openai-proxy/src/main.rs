use bytes::{Buf, Bytes};
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use http_body_util::Limited;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1::Builder as ServerBuilder;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use reqwest::Client;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8100));

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = ServerBuilder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(proxy))
                .with_upgrades()
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
pub async fn proxy(
    req: Request<Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    println!("Request headers: {:?}", req);
    if req.headers().get(AUTHORIZATION).is_none() {
        return Ok(Response::builder()
            .status(401)
            .body(full("Missing Authorization header"))
            .unwrap());
    }

    let (parts, body) = req.into_parts();
    let client = Client::new();
    let body_bytes = match read_body(&parts.headers, body, 64 * 1024 * 1024).await {
        Ok(body) => body,
        Err(err) => {
            return Ok(Response::builder()
                .status(500)
                .body(full(err.to_string()))
                .unwrap());
        }
    };

    let url = format!("https://api.openai.com{}", parts.uri.path());
    println!("Request URL: {}, method: {}", url, parts.method);

    let mut request_builder = client.request(parts.method.clone(), url);

    if let Some(auth) = parts.headers.get(AUTHORIZATION) {
        request_builder = request_builder.header(AUTHORIZATION, auth);
    }

    // 补充 User-Agent 和 Host
    request_builder = request_builder
        .header("Host", "api.openai.com")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "*/*")
        .header(USER_AGENT, "openai-proxy/0.1"); // 伪造一个标准User-Agent;

    let response = match request_builder.body(body_bytes).send().await {
        Ok(resp) => resp,
        Err(err) => {
            println!("Failed to send request: {:?}", err);
            return Ok(Response::builder()
                .status(500)
                .body(full(err.to_string()))
                .unwrap());
        }
    };

    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.unwrap_or_else(|_| Bytes::new());

    let mut resp_builder = Response::builder().status(status);

    for (key, value) in headers.iter() {
        resp_builder = resp_builder.header(key, value);
    }

    Ok(resp_builder
        .body(Full::new(body).map_err(|err| match err {}).boxed())
        .unwrap())
}

fn read_header_content_length(headers: &http::header::HeaderMap) -> Option<u32> {
    let length = read_header_value(headers, http::header::CONTENT_LENGTH)?;
    // HTTP Content-Length indicates number of bytes in decimal.
    length.parse::<u32>().ok()
}

/// Returns a string value when there is exactly one value for the given header.
pub fn read_header_value(
    headers: &http::header::HeaderMap,
    header_name: http::header::HeaderName,
) -> Option<&str> {
    let mut values = headers.get_all(header_name).iter();
    let val = values.next()?;
    if values.next().is_none() {
        val.to_str().ok()
    } else {
        None
    }
}

pub async fn read_body<B>(
    headers: &http::HeaderMap,
    body: B,
    max_body_size: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>
where
    B: http_body::Body<Data = Bytes> + Send,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    // 读取Content-Length，默认为0
    let body_size = read_header_content_length(headers).unwrap_or(0);

    futures::pin_mut!(body);

    // 为响应体分配一个初始缓冲区（16KB或小于body_size的值）
    let mut received_data = Vec::with_capacity(std::cmp::min(body_size as usize, 16 * 1024));

    // 使用 Limited 体读取器，确保体积不超过最大限制
    let mut limited_body = Limited::new(body, max_body_size as usize);

    while let Some(frame_or_err) = limited_body.frame().await {
        let frame = frame_or_err?;
        let Some(data) = frame.data_ref() else {
            continue;
        };
        received_data.extend_from_slice(data.chunk());
    }
    Ok(received_data)
}
