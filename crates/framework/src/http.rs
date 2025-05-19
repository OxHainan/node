use crate::context::StateDiff;
use httparse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// HTTP方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Options,
    Head,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Options => write!(f, "OPTIONS"),
            HttpMethod::Head => write!(f, "HEAD"),
        }
    }
}

// HTTP请求
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: String,
}

impl HttpRequest {
    pub fn new(method: HttpMethod, path: &str) -> Self {
        Self {
            method,
            path: path.to_string(),
            headers: HashMap::new(),
            query: HashMap::new(),
            body: String::new(),
        }
    }

    pub fn with_body(mut self, body: &str) -> Self {
        self.body = body.to_string();
        self
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_query(mut self, key: &str, value: &str) -> Self {
        self.query.insert(key.to_string(), value.to_string());
        self
    }

    // 从 TCP 流中解析 HTTP 请求
    pub async fn from_stream<T>(stream: &mut T) -> Result<Self, crate::errors::Error>
    where
        T: tokio::io::AsyncRead + Unpin,
    {
        use std::io::ErrorKind;
        use tokio::io::AsyncReadExt;

        // 读取请求数据
        let mut buffer = [0; 4096];
        let n = match stream.read(&mut buffer).await {
            Ok(n) => n,
            Err(e) => {
                return Err(crate::errors::Error::HttpError(format!(
                    "Error reading request: {}",
                    e
                )))
            }
        };

        if n == 0 {
            return Err(crate::errors::Error::HttpError("Empty request".to_string()));
        }

        // 使用httparse解析请求
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);

        let status = match req.parse(&buffer[..n]) {
            Ok(status) => status,
            Err(e) => {
                return Err(crate::errors::Error::HttpError(format!(
                    "Error parsing request: {}",
                    e
                )))
            }
        };

        // 获取方法
        let method = match req.method {
            Some("GET") => HttpMethod::Get,
            Some("POST") => HttpMethod::Post,
            Some("PUT") => HttpMethod::Put,
            Some("DELETE") => HttpMethod::Delete,
            Some("OPTIONS") => HttpMethod::Options,
            Some("HEAD") => HttpMethod::Head,
            Some(m) => {
                return Err(crate::errors::Error::HttpError(format!(
                    "Unsupported HTTP method: {}",
                    m
                )))
            }
            None => {
                return Err(crate::errors::Error::HttpError(
                    "Missing HTTP method".to_string(),
                ))
            }
        };

        // 获取路径
        let path = match req.path {
            Some(p) => p.to_string(),
            None => return Err(crate::errors::Error::HttpError("Missing path".to_string())),
        };

        // 解析路径和查询参数
        let mut path_without_query = path.clone();
        let mut query_params = HashMap::new();

        if let Some(query_start) = path.find('?') {
            path_without_query = path[..query_start].to_string();
            let query_str = &path[query_start + 1..];

            // 解析查询参数
            for pair in query_str.split('&') {
                if let Some(eq_pos) = pair.find('=') {
                    let key = pair[..eq_pos].to_string();
                    let value = pair[eq_pos + 1..].to_string();
                    query_params.insert(key, value);
                }
            }
        }

        // 解析请求头
        let mut headers_map = HashMap::new();
        for header in req.headers.iter() {
            let value = match std::str::from_utf8(header.value) {
                Ok(v) => v.to_string(),
                Err(_) => continue,
            };
            headers_map.insert(header.name.to_lowercase(), value);
        }

        // 获取请求体
        let mut body = String::new();
        if let Some(content_length_str) = headers_map.get("content-length") {
            if let Ok(content_length) = content_length_str.parse::<usize>() {
                // 计算请求体的起始位置
                let body_start = match status {
                    httparse::Status::Complete(pos) => pos,
                    httparse::Status::Partial => 0, // 如果是部分解析，假设没有请求体
                };

                if body_start + content_length <= n {
                    body =
                        String::from_utf8_lossy(&buffer[body_start..body_start + content_length])
                            .to_string();
                }
            }
        }

        // 创建请求对象
        let mut request = HttpRequest::new(method, &path_without_query);
        request.headers = headers_map;
        request.query = query_params;
        request.body = body;

        Ok(request)
    }
}

// HTTP状态码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    InternalServerError = 500,
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = *self as u16;
        write!(f, "{}", code)
    }
}

// HTTP响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse<T> {
    pub status: u16,
    pub data: Option<T>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_diff: Option<StateDiff>,
}

impl<T: Serialize> HttpResponse<T> {
    // 创建成功响应
    pub fn ok(data: T) -> Self {
        Self {
            status: StatusCode::Ok as u16,
            data: Some(data),
            error: None,
            transaction_id: None,
            state_diff: None,
        }
    }

    // 创建创建成功响应
    pub fn created(data: T) -> Self {
        Self {
            status: StatusCode::Created as u16,
            data: Some(data),
            error: None,
            transaction_id: None,
            state_diff: None,
        }
    }

    // 创建未找到响应
    pub fn not_found(error_message: &str) -> Self
    where
        T: Default,
    {
        Self {
            status: StatusCode::NotFound as u16,
            data: None,
            error: Some(error_message.to_string()),
            transaction_id: None,
            state_diff: None,
        }
    }

    // 创建错误请求响应
    pub fn bad_request(error_message: &str) -> Self
    where
        T: Default,
    {
        Self {
            status: StatusCode::BadRequest as u16,
            data: None,
            error: Some(error_message.to_string()),
            transaction_id: None,
            state_diff: None,
        }
    }

    // 设置交易ID
    pub fn with_transaction_id(mut self, transaction_id: &str) -> Self {
        self.transaction_id = Some(transaction_id.to_string());
        self
    }

    // 获取交易ID
    pub fn transaction_id(&self) -> Option<&str> {
        self.transaction_id.as_deref()
    }

    // 设置状态变更
    pub fn with_state_diff(mut self, state_diff: StateDiff) -> Self {
        self.state_diff = Some(state_diff);
        self
    }

    // 转换为JSON字符串
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    // 发送HTTP响应
    pub async fn send<W>(&self, stream: &mut W) -> Result<(), crate::errors::Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        // 将响应序列化为JSON
        let json = match serde_json::to_string(self) {
            Ok(json) => json,
            Err(e) => return Err(crate::errors::Error::SerializationError(e.to_string())),
        };

        // 构建HTTP响应头
        let status_text = match self.status {
            200 => "OK",
            201 => "Created",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        };

        // 构建完整的HTTP响应
        let response = format!(
            "HTTP/1.1 {} {}\r\n\
            Content-Type: application/json\r\n\
            Content-Length: {}\r\n\
            Access-Control-Allow-Origin: *\r\n\
            \r\n\
            {}",
            self.status,
            status_text,
            json.len(),
            json
        );

        // 发送响应
        use tokio::io::AsyncWriteExt;
        match stream.write_all(response.as_bytes()).await {
            Ok(_) => Ok(()),
            Err(e) => Err(crate::errors::Error::IoError(e.to_string())),
        }
    }
}

// HTTP错误响应
pub struct HttpError {
    pub status: StatusCode,
    pub message: String,
}

impl HttpError {
    // 创建错误响应
    pub fn new(status: StatusCode, message: &str) -> Self {
        Self {
            status,
            message: message.to_string(),
        }
    }

    // 创建400错误
    pub fn bad_request(message: &str) -> Self {
        Self::new(StatusCode::BadRequest, message)
    }

    // 创建404错误
    pub fn not_found(message: &str) -> Self {
        Self::new(StatusCode::NotFound, message)
    }

    // 创建500错误
    pub fn internal_error(message: &str) -> Self {
        Self::new(StatusCode::InternalServerError, message)
    }

    // 转换为HTTP响应
    pub fn to_response<T>(&self) -> HttpResponse<T> {
        HttpResponse {
            status: self.status as u16,
            data: None,
            error: Some(self.message.clone()),
            transaction_id: None,
            state_diff: None,
        }
    }
}
