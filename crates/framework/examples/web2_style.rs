use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use mp_derive::mpModel;
use mp_framework::{
    adapters::DatabaseProxy,
    context::Context,
    database::Database,
    errors::{Error, Result},
    model::Model,
    server::Server,
    state_serialize::{Identifiable, StateSerializable},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

// ==================== 1. 模型层（Model）====================

// 用户模型
#[derive(Debug, Clone, Serialize, Deserialize, mpModel)]
#[mp(table = "users", id = "id")]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 帖子模型
#[derive(Debug, Clone, Serialize, Deserialize, mpModel)]
#[mp(table = "posts", id = "id")]
pub struct Post {
    pub id: String,
    pub title: String,
    pub content: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 用户创建请求
#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    id: String,
    name: String,
    email: String,
}

// 帖子创建请求
#[derive(Debug, Deserialize)]
struct CreatePostRequest {
    id: String,
    title: String,
    content: String,
    user_id: String,
}

// ==================== 2. 服务层（Service）====================

// 用户服务 - 包含用户相关的业务逻辑
struct UserService<'a> {
    db: &'a Database,
    ctx: &'a mut Context,
}

impl<'a> UserService<'a> {
    // 创建用户服务
    fn new(db: &'a Database, ctx: &'a mut Context) -> Self {
        Self { db, ctx }
    }

    // 创建用户 - 包含业务逻辑
    fn create_user(&mut self, request: CreateUserRequest) -> Result<User> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        info!("Checking if user already exists. User ID: {}", request.id);
        // 检查用户是否已存在
        if db_proxy.get::<User>(&request.id)?.is_some() {
            warn!(
                "Failed to create user: User ID already exists. User ID: {}",
                request.id
            );
            return Err(Error::BadRequest(format!(
                "User with ID {} already exists",
                request.id
            )));
        }

        info!(
            "Creating new user. User ID: {}, Name: {}",
            request.id, request.name
        );
        // 创建用户
        let user = User {
            id: request.id,
            name: request.name,
            email: request.email,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 保存用户
        info!("Saving new user to database. User ID: {}", user.id);
        db_proxy.save(&user)?;
        info!(
            "Successfully created user. User ID: {}, Name: {}, Email: {}",
            user.id, user.name, user.email
        );

        Ok(user)
    }

    // 获取用户 - 包含业务逻辑
    fn get_user(&mut self, id: String) -> Result<User> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        info!("Attempting to retrieve user. User ID: {}", id);
        // 查询用户
        if let Some(user) = db_proxy.get::<User>(&id)? {
            info!(
                "Successfully retrieved user. User ID: {}, Name: {}",
                user.id, user.name
            );
            Ok(user)
        } else {
            warn!("Failed to retrieve user: User not found. User ID: {}", id);
            Err(Error::NotFound(format!("User with ID {} not found", id)))
        }
    }

    // 获取所有用户 - 包含业务逻辑
    fn get_all_users(&mut self) -> Result<Vec<User>> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        info!("Retrieving all users from database");
        // 查询所有用户
        let users = db_proxy.get_all::<User>()?;
        info!("Successfully retrieved {} users from database", users.len());

        Ok(users)
    }

    // 更新用户
    fn update_user(
        &mut self,
        id: String,
        name: Option<String>,
        email: Option<String>,
    ) -> Result<User> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        // 查询用户
        let mut user = {
            // 查询用户
            if let Some(user) = db_proxy.get::<User>(&id)? {
                user
            } else {
                return Err(Error::NotFound(format!("User with ID {} not found", id)));
            }
        };

        // 更新用户信息
        if let Some(name) = name {
            user.name = name;
        }

        if let Some(email) = email {
            user.email = email;
        }

        user.updated_at = Utc::now();

        // 保存用户
        db_proxy.save(&user)?;
        info!("Successfully created user with ID: {}", user.id);

        Ok(user)
    }

    // 删除用户
    fn delete_user(&mut self, id: String) -> Result<()> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        // 查询用户
        let user = {
            // 查询用户
            if let Some(user) = db_proxy.get::<User>(&id)? {
                user
            } else {
                return Err(Error::NotFound(format!("User with ID {} not found", id)));
            }
        };

        // 删除用户
        db_proxy.delete(&user)?;

        Ok(())
    }
}

// 帖子服务 - 包含帖子相关的业务逻辑
struct PostService<'a> {
    db: &'a Database,
    ctx: &'a mut Context,
}

impl<'a> PostService<'a> {
    // 创建帖子服务
    fn new(db: &'a Database, ctx: &'a mut Context) -> Self {
        Self { db, ctx }
    }

    // 创建帖子 - 包含业务逻辑
    fn create_post(&mut self, request: CreatePostRequest) -> Result<Post> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        info!(
            "Checking if user exists for post creation. User ID: {}",
            request.user_id
        );
        // 检查用户是否存在
        if db_proxy.get::<User>(&request.user_id)?.is_none() {
            warn!(
                "Failed to create post: User not found. User ID: {}",
                request.user_id
            );
            return Err(Error::NotFound(format!(
                "User with ID {} not found",
                request.user_id
            )));
        }

        info!("Checking if post already exists. Post ID: {}", request.id);
        // 检查帖子是否已存在
        if db_proxy.get::<Post>(&request.id)?.is_some() {
            warn!(
                "Failed to create post: Post ID already exists. Post ID: {}",
                request.id
            );
            return Err(Error::BadRequest(format!(
                "Post with ID {} already exists",
                request.id
            )));
        }

        // 创建帖子
        let post = Post {
            id: request.id,
            title: request.title,
            content: request.content,
            user_id: request.user_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        info!(
            "Saving new post to database. Post ID: {}, Title: {}",
            post.id, post.title
        );
        // 保存帖子
        db_proxy.save(&post)?;
        info!(
            "Successfully created post. Post ID: {}, Title: {}, User ID: {}",
            post.id, post.title, post.user_id
        );

        Ok(post)
    }

    // 获取帖子 - 包含业务逻辑
    fn get_post(&mut self, id: String) -> Result<Post> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        info!("Attempting to retrieve post. Post ID: {}", id);
        // 查询帖子
        if let Some(post) = db_proxy.get::<Post>(&id)? {
            info!(
                "Successfully retrieved post. Post ID: {}, Title: {}",
                post.id, post.title
            );
            Ok(post)
        } else {
            warn!("Failed to retrieve post: Post not found. Post ID: {}", id);
            Err(Error::NotFound(format!("Post with ID {} not found", id)))
        }
    }

    // 获取用户的所有帖子 - 包含业务逻辑
    fn get_user_posts(&mut self, user_id: String) -> Result<Vec<Post>> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        info!(
            "Checking if user exists before retrieving posts. User ID: {}",
            user_id
        );
        // 检查用户是否存在
        if db_proxy.get::<User>(&user_id)?.is_none() {
            warn!(
                "Failed to retrieve user posts: User not found. User ID: {}",
                user_id
            );
            return Err(Error::NotFound(format!(
                "User with ID {} not found",
                user_id
            )));
        }

        info!("Retrieving all posts for user. User ID: {}", user_id);
        // 查询用户的所有帖子
        let posts = db_proxy.find_where::<Post, _>("posts", |post| post.user_id == user_id)?;
        info!(
            "Successfully retrieved {} posts for user. User ID: {}",
            posts.len(),
            user_id
        );

        Ok(posts)
    }

    // 更新帖子
    fn update_post(
        &mut self,
        id: String,
        title: Option<String>,
        content: Option<String>,
    ) -> Result<Post> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        // 查询帖子
        let mut post = {
            // 查询帖子
            if let Some(post) = db_proxy.get::<Post>(&id)? {
                post
            } else {
                return Err(Error::NotFound(format!("Post with ID {} not found", id)));
            }
        };

        // 更新帖子信息
        if let Some(title) = title {
            post.title = title;
        }

        if let Some(content) = content {
            post.content = content;
        }

        post.updated_at = Utc::now();

        // 保存帖子
        db_proxy.save(&post)?;

        Ok(post)
    }

    // 删除帖子
    fn delete_post(&mut self, id: String) -> Result<()> {
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

        // 查询帖子
        let post = {
            // 查询帖子
            if let Some(post) = db_proxy.get::<Post>(&id)? {
                post
            } else {
                return Err(Error::NotFound(format!("Post with ID {} not found", id)));
            }
        };

        // 删除帖子
        db_proxy.delete(&post)?;

        Ok(())
    }
}

// ==================== 3. 控制器层（Controller）====================

// 应用状态
#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
}

// 创建用户处理器
async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut ctx = Context::new();
    let mut user_service = UserService::new(&state.db, &mut ctx);

    // 记录创建用户请求
    info!(
        "Creating user with ID: {}, name: {}, email: {}",
        payload.id, payload.name, payload.email
    );

    // 调用服务层创建用户
    let user = user_service.create_user(payload)?;

    // 记录用户创建成功
    info!("Successfully created user with ID: {}", user.id);

    // 构建响应
    let mut response = serde_json::json!({
        "status_code": 201,
        "transaction_id": ctx.transaction_id,
        "state_diffs": ctx.state_diffs,
        "entity_diffs": ctx.entity_diffs,
    });

    // 添加用户数据到响应
    if let Ok(user_json) = serde_json::to_value(&user) {
        response["user"] = user_json;
    }

    Ok((StatusCode::CREATED, Json(response)))
}

// 获取用户处理器
async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut ctx = Context::new();
    let mut user_service = UserService::new(&state.db, &mut ctx);

    // 记录获取用户请求
    info!("Getting user with ID: {}", id);

    // 调用服务层获取用户
    match user_service.get_user(id) {
        Ok(user) => {
            // 构建响应
            let response = serde_json::json!({
                "status_code": 200,
                "user": user,
            });

            Ok((StatusCode::OK, Json(response)))
        }
        Err(Error::NotFound(msg)) => {
            // 用户不存在
            let response = serde_json::json!({
                "status_code": 404,
                "error": msg,
            });

            Ok((StatusCode::NOT_FOUND, Json(response)))
        }
        Err(e) => Err(e),
    }
}

// 获取所有用户处理器
async fn get_all_users(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut ctx = Context::new();
    let mut user_service = UserService::new(&state.db, &mut ctx);

    // 记录获取所有用户请求
    info!("Getting all users");

    // 调用服务层获取所有用户
    let users = user_service.get_all_users()?;

    // 记录获取用户成功
    info!("Successfully retrieved {} users", users.len());

    // 构建响应
    let response = serde_json::json!({
        "status_code": 200,
        "users": users,
    });

    Ok((StatusCode::OK, Json(response)))
}

// 创建帖子处理器
async fn create_post(
    State(state): State<AppState>,
    Json(payload): Json<CreatePostRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut ctx = Context::new();
    let mut post_service = PostService::new(&state.db, &mut ctx);

    // 记录创建帖子请求
    info!(
        "Creating post with ID: {}, title: {}, user_id: {}",
        payload.id, payload.title, payload.user_id
    );

    // 调用服务层创建帖子
    match post_service.create_post(payload) {
        Ok(post) => {
            // 构建响应
            let mut response = serde_json::json!({
                "status_code": 201,
                "transaction_id": ctx.transaction_id,
                "state_diffs": ctx.state_diffs,
                "entity_diffs": ctx.entity_diffs,
            });

            // 添加帖子数据到响应
            if let Ok(post_json) = serde_json::to_value(&post) {
                response["post"] = post_json;
            }

            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(Error::NotFound(msg)) => {
            // 用户不存在
            let response = serde_json::json!({
                "status_code": 404,
                "error": msg,
            });

            Ok((StatusCode::NOT_FOUND, Json(response)))
        }
        Err(e) => Err(e),
    }
}

// 获取帖子处理器
async fn get_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut ctx = Context::new();
    let mut post_service = PostService::new(&state.db, &mut ctx);

    // 记录获取帖子请求
    info!("Getting post with ID: {}", id);

    // 调用服务层获取帖子
    match post_service.get_post(id) {
        Ok(post) => {
            // 构建响应
            let response = serde_json::json!({
                "status_code": 200,
                "post": post,
            });

            Ok((StatusCode::OK, Json(response)))
        }
        Err(Error::NotFound(msg)) => {
            // 帖子不存在
            let response = serde_json::json!({
                "status_code": 404,
                "error": msg,
            });

            Ok((StatusCode::NOT_FOUND, Json(response)))
        }
        Err(e) => Err(e),
    }
}

// 获取用户的所有帖子处理器
async fn get_user_posts(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut ctx = Context::new();
    let mut post_service = PostService::new(&state.db, &mut ctx);

    // 记录获取用户帖子请求
    info!("Getting posts for user with ID: {}", user_id);

    // 调用服务层获取用户帖子
    match post_service.get_user_posts(user_id) {
        Ok(posts) => {
            // 构建响应
            let response = serde_json::json!({
                "status_code": 200,
                "posts": posts,
            });

            Ok((StatusCode::OK, Json(response)))
        }
        Err(Error::NotFound(msg)) => {
            // 用户不存在
            let response = serde_json::json!({
                "status_code": 404,
                "error": msg,
            });

            Ok((StatusCode::NOT_FOUND, Json(response)))
        }
        Err(e) => Err(e),
    }
}

// ==================== 4. 主函数 ====================

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .compact()
        .init();

    // 创建内存数据库
    let db = Arc::new(Database::new_in_memory()?);

    // 创建应用状态
    let app_state = AppState { db };

    // 创建路由
    let app = Router::new()
        .route("/users", post(create_user))
        .route("/users", get(get_all_users))
        .route("/users/:id", get(get_user))
        .route("/posts", post(create_post))
        .route("/posts/:id", get(get_post))
        .route("/users/:user_id/posts", get(get_user_posts))
        .with_state(app_state);

    // 启动服务器
    let server = Server::new().with_router(app);
    server.start("0.0.0.0:8080").await?;

    Ok(())
}
