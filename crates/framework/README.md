# mp Framework

mp Framework 是一个创新的智能合约开发框架，为区块链开发者提供类似 Web2 的开发体验，同时保留区块链的核心特性，如状态追踪和事务管理。

## 设计理念

mp Framework 致力于解决区块链开发中的两大挑战：

1. **开发体验差距** - 传统智能合约开发体验与现代 Web 应用程序开发相差甚远
2. **状态管理复杂性** - 区块链状态管理需要开发者编写大量样板代码

通过提供熟悉的 Web2 开发模式和自动化的状态追踪机制，mp Framework 显著降低了区块链开发的学习曲线和复杂性。

## 核心架构

### 容器内执行模型

mp 采用容器内执行架构，具有以下关键特点：

1. **内存数据库执行** - 合约在容器内使用内存数据库执行，确保高性能和隔离性
2. **自动状态跟踪** - 所有状态变更由框架自动跟踪，无需手动管理
3. **主动通知机制** - 执行完成后，状态变更通过通知系统返回给区块链节点
4. **共识后持久化** - 节点达成共识后将状态变更持久化到专用数据库空间


### 模块结构

mp Framework 由以下核心模块组成：

1. **模型系统 (Model)** - 定义实体结构和数据库交互
2. **上下文管理 (Context)** - 管理事务和状态跟踪
3. **数据库接口 (Database)** - 提供统一的数据访问层
4. **状态序列化 (StateSerialize)** - 处理实体的序列化和反序列化
5. **服务层 (Service)** - 实现业务逻辑
6. **控制器 (Controller)** - 处理外部请求
7. **适配器 (Adapters)** - 简化组件间的交互

## 关键技术亮点

### 自动派生宏

使用 `mpModel` 派生宏，开发者只需几行代码即可自动实现 `Model`、`Identifiable` 和 `StateSerializable` 特性：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, mpModel)]
#[mp(table = "users", id = "id")]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 透明状态跟踪

状态变更由框架自动跟踪，开发者无需手动管理：

```rust
// 保存用户 - 状态变更自动被追踪
db_proxy.save(&user)?;
```

每个操作都会在 `Context` 中记录状态差异：

```rust
{
  "transaction_id": "8f7d1a63-e4b2-4782-b87f-9a0e36a9b7c1",
  "state_diffs": [...],
  "entity_diffs": [
    {
      "entity_type": "User",
      "id": "user_123",
      "action": "update",
      "data": { ... }
    }
  ]
}
```

### 服务层抽象

服务层包含业务逻辑，与控制器和数据库访问解耦：

```rust
// 用户服务 - 包含业务逻辑
struct UserService<'a> {
    db: &'a Database,
    ctx: &'a mut Context,
}

impl<'a> UserService<'a> {
    // 创建用户 - 包含业务逻辑
    fn create_user(&mut self, request: CreateUserRequest) -> Result<User> {
        // 业务逻辑实现...
    }
}
```

### Axum 集成

mp Framework 无缝集成 Axum Web 框架，提供现代化的路由和请求处理：

```rust
// 创建路由
let app = Router::new()
    .route("/users", post(create_user))
    .route("/users", get(get_all_users))
    .route("/users/:id", get(get_user))
    .with_state(app_state);
```

## 开发智能合约：完整指南

以下是使用 mp Framework 开发智能合约的完整流程，以 `web2_style.rs` 为例：

### 步骤 1：定义模型

使用 `mpModel` 派生宏定义开发者的实体模型：

```rust
// 定义用户模型
#[derive(Debug, Clone, Serialize, Deserialize, mpModel)]
#[mp(table = "users", id = "id")]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 定义请求数据结构
#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    id: String,
    name: String,
    email: String,
}
```

### 步骤 2：实现服务层

服务层封装业务逻辑，与数据访问和控制器解耦：

```rust
// 用户服务 - 包含业务逻辑
struct UserService<'a> {
    db: &'a Database,        // 数据库引用
    ctx: &'a mut Context,    // 上下文（状态跟踪）
}

impl<'a> UserService<'a> {
    // 创建服务实例
    fn new(db: &'a Database, ctx: &'a mut Context) -> Self {
        Self { db, ctx }
    }
    
    // 创建用户 - 包含验证和业务规则
    fn create_user(&mut self, request: CreateUserRequest) -> Result<User> {
        // 1. 创建数据库代理
        let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);
        
        // 2. 验证业务规则
        if db_proxy.get::<User>(&request.id)?.is_some() {
            return Err(Error::BadRequest(format!("User already exists")));
        }
        
        // 3. 创建实体
        let user = User {
            id: request.id,
            name: request.name,
            email: request.email,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // 4. 保存实体（状态自动跟踪）
        db_proxy.save(&user)?;
        
        // 5. 返回结果
        Ok(user)
    }
    
    // 其他业务方法...
}
```

在mp框架中，实现像这样的服务层并不是严格要求的。开发者确实可以在`ctx`和`DatabaseProxy`周围封装业务逻辑，而不一定需要将其结构化为专门的服务。

以下是一些相关的考虑因素：

1. **框架灵活性**：mp框架允许开发者在代码结构上有一定的灵活性。虽然示例使用服务层来提高清晰度和组织性，但开发者可以在路由处理程序或其他组件中直接实现逻辑，只要遵循框架的API接口。

2. **封装性**：使用服务层可以帮助封装业务逻辑，并使其在应用程序的不同部分之间可重用。然而，如果开发者的应用程序比较简单，或者开发者更喜欢直接的方法，开发者可以在路由处理程序中直接处理`ctx`和`DatabaseProxy`。

3. **接口要求**：框架的要求主要集中在开发者如何与其组件（如`ctx`和`DatabaseProxy`）进行交互。只要开发者正确使用这些组件，就可以以最适合开发者应用程序需求的方式实现逻辑。

所以，**虽然服务层是组织和可维护性的良好实践，但它并不是mp框架的严格要求**。如果这种结构更符合开发者的设计偏好，开发者可以采用更直接的方式来实现业务逻辑。

### 步骤 3：实现控制器

控制器处理外部请求，调用服务层，并格式化响应：

```rust
// 创建用户控制器
async fn create_user(
    State(state): State<AppState>,            // 应用状态
    Json(payload): Json<CreateUserRequest>,   // 请求体
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    // 1. 创建上下文
    let mut ctx = Context::new();
    
    // 2. 创建服务实例
    let mut user_service = UserService::new(&state.db, &mut ctx);

    // 3. 调用服务层
    let user = user_service.create_user(payload)?;

    // 4. 构建响应
    let response = serde_json::json!({
        "status_code": 201,
        "transaction_id": ctx.transaction_id,       // 事务ID
        "state_diffs": ctx.state_diffs,             // 状态差异
        "entity_diffs": ctx.entity_diffs,           // 实体变更
        "user": user
    });

    // 5. 返回结果
    Ok((StatusCode::CREATED, Json(response)))
}
```

在mp框架中，控制器的实现也并不是严格要求的。与服务层类似，开发者可以在控制器中直接处理业务逻辑，而不一定需要将其结构化为专门的控制器。

以下是一些关于控制器的考虑因素：

- 灵活性：mp框架允许开发者根据需要灵活地实现控制器。开发者可以在控制器中直接使用`ctx`和`DatabaseProxy`，而不必将逻辑封装在单独的控制器结构中。
- 简化实现：对于简单的应用程序，直接在控制器中实现逻辑可以减少代码复杂性，使代码更易于理解和维护。
- 接口要求：与服务层一样，控制器的实现应遵循框架的API接口要求。只要开发者正确使用框架提供的组件，开发者可以自由地选择实现方式。

所以，**虽然将业务逻辑封装在控制器中是一个良好的实践，但在mp框架中也不是强制要求**。开发者可以根据应用程序的需求和复杂性选择最适合的实现方式。

### 步骤 4：注册路由和启动服务器

将控制器与路由关联，并启动服务器：

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建内存数据库
    let db = Arc::new(Database::new_in_memory()?);

    // 2. 创建应用状态
    let app_state = AppState { db };

    // 3. 注册路由
    let app = Router::new()
        .route("/users", post(create_user))
        .route("/users", get(get_all_users))
        .route("/users/:id", get(get_user))
        .route("/posts", post(create_post))
        .route("/posts/:id", get(get_post))
        .route("/users/:user_id/posts", get(get_user_posts))
        .with_state(app_state);

    // 4. 启动服务器
    let server = Server::new().with_router(app);
    server.start("127.0.0.1:8081").await?;

    Ok(())
}
```

## 关键组件详解

### Context (上下文)

`Context` 是 mp Framework 的核心组件，负责状态跟踪和事务管理：

```rust
pub struct Context {
    /// 事务ID
    pub transaction_id: String,
    
    /// 状态变更
    pub state_diffs: Vec<StateDiff>,
    
    /// 实体变更
    pub entity_diffs: Vec<EntityDiff>,
    
    // 内部状态...
}
```

每个请求都会创建一个新的 `Context` 实例，用于跟踪状态变更。

### DatabaseProxy (数据库代理)

`DatabaseProxy` 简化了数据库操作，自动处理状态跟踪：

```rust
// 创建数据库代理
let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);

// 保存实体（自动跟踪状态）
db_proxy.save(&user)?;

// 获取实体
let user = db_proxy.get::<User>(&id)?;

// 删除实体（自动跟踪状态）
db_proxy.delete(&user)?;
```

### mpModel 派生宏

`mpModel` 派生宏自动实现多个特性，简化模型定义：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, mpModel)]
#[mp(table = "users", id = "id")]
pub struct User { /* ... */ }

// 自动实现：
// - Model：基本模型操作
// - Identifiable：ID 和实体类型
// - StateSerializable：状态序列化
```

## 关系管理

mp Framework 支持实体之间的关系管理：

```rust
// 查询用户的所有帖子
fn get_user_posts(&mut self, user_id: String) -> Result<Vec<Post>> {
    let mut db_proxy = DatabaseProxy::new(self.db, self.ctx);
    
    // 检查用户是否存在
    if db_proxy.get::<User>(&user_id)?.is_none() {
        return Err(Error::NotFound(format!("User not found")));
    }
    
    // 查询关联实体
    let posts = db_proxy.find_where::<Post, _>("posts", |post| {
        post.user_id == user_id
    })?;
    
    Ok(posts)
}
```

## 错误处理

mp Framework 提供全面的错误处理机制：

```rust
// 框架错误类型
pub enum Error {
    DatabaseError(String),
    HttpError(String),
    SerializationError(String),
    DeserializationError(String),
    ValidationError(String),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    InternalError(String),
    BlockchainError(String),
    IoError(String),
    NotificationError(String),
    NotImplemented(String),
}

// Axum 集成
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // 将错误转换为 HTTP 响应...
    }
}
```

## 测试指南

mp Framework 设计了便于测试的架构：

1. **单元测试** - 测试单个组件的功能
2. **集成测试** - 测试组件之间的交互
3. **模拟测试** - 使用内存数据库进行快速测试

### 系统测试示例

通过命令行启动 @[crates/mp-framework/examples/web2_style.rs] 这个合约示例，您可以按照以下步骤操作：

1. 编译并运行合约示例： 打开终端，导航到合约示例所在的目录，然后使用以下命令编译并运行合约：
```bash
cargo run --example web2_style
```
这将启动合约示例并在终端中显示相关输出。

2. 另起一个终端运行测试脚本： 打开另一个终端窗口，导航到同样的目录，然后运行测试脚本以与前面启动的合约进程进行通信：
```bash
./test_web2_style.sh
```
该脚本将与合约进程进行交互，执行预定义的测试用例。
通过以上步骤，您可以成功启动合约示例并运行测试脚本。

期待的输出如下：
```bash
$ curl -X POST -H "Content-Type: application/json" -d '{"id": "user1", "name": "张三", "email": "zhangsan@example.com"}' http://127.0.0.1:8080/users

{"entity_diffs":[{"action":"update","data":{"created_at":"2025-03-17T10:25:26.790175Z","email":"zhangsan@example.com","id":"user1","name":"张三","updated_at":"2025-03-17T10:25:26.790185Z"},"entity_type":"users","id":"user1"}],"state_diffs":[{"key":"users/user1/created_at","new_value":"2025-03-17T10:25:26.790175Z","old_value":null},{"key":"users/user1/email","new_value":"zhangsan@example.com","old_value":null},{"key":"users/user1/id","new_value":"user1","old_value":null},{"key":"users/user1/name","new_value":"张三","old_value":null},{"key":"users/user1/updated_at","new_value":"2025-03-17T10:25:26.790185Z","old_value":null}],"status_code":201,"transaction_id":"b0d59649-9f0f-430d-b39b-b9b93da172a6","user":{"created_at":"2025-03-17T10:25:26.790175Z","email":"zhangsan@example.com","id":"user1","name":"张三","updated_at":"2025-03-17T10:25:26.790185Z"}}

$ curl -X POST -H "Content-Type: application/json" -d '{"id": "post1", "user_id": "user1", "title": "我的第一篇帖 子", "content": "这是我的第一篇帖子内容，大家好！"}' http://127.0.0.1:8080/posts

{"entity_diffs":[{"action":"update","data":{"content":"这是我的第一篇帖子内容，大家好！","created_at":"2025-03-17T10:25:35.362497Z","id":"post1","title":"我的第一篇帖子","updated_at":"2025-03-17T10:25:35.362497Z","user_id":"user1"},"entity_type":"posts","id":"post1"}],"post":{"content":"这是我的第一篇帖子内容，大家好！","created_at":"2025-03-17T10:25:35.362497Z","id":"post1","title":"我的第一篇帖子","updated_at":"2025-03-17T10:25:35.362497Z","user_id":"user1"},"state_diffs":[{"key":"posts/post1/content","new_value":"这是我的第一篇帖子内容，大家好！","old_value":null},{"key":"posts/post1/created_at","new_value":"2025-03-17T10:25:35.362497Z","old_value":null},{"key":"posts/post1/id","new_value":"post1","old_value":null},{"key":"posts/post1/title","new_value":"我的第一篇帖子","old_value":null},{"key":"posts/post1/updated_at","new_value":"2025-03-17T10:25:35.362497Z","old_value":null},{"key":"posts/post1/user_id","new_value":"user1","old_value":null}],"status_code":201,"transaction_id":"0ba8fc64-3224-4d15-8991-6b249bb9ac1d"}
```

### 单元测试示例
当前并没有提供单元测试示例，开发者可考虑采用如下方式编写单元测试实例。

```rust
#[test]
fn test_create_user() {
    // 创建测试环境
    let db = Database::in_memory();
    let mut ctx = Context::new();
    
    // 创建服务实例
    let mut user_service = UserService::new(&db, &mut ctx);
    
    // 执行测试
    let request = CreateUserRequest {
        id: "test_user".to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
    };
    
    let result = user_service.create_user(request);
    assert!(result.is_ok());
    
    // 验证状态变更
    assert!(!ctx.entity_diffs.is_empty());
}
```

## 最佳实践

1. **使用派生宏** - 尽可能使用 `mpModel` 派生宏简化代码
2. **服务层封装** - 将业务逻辑封装在服务层，保持控制器简洁
3. **状态跟踪** - 利用自动状态跟踪，不要手动管理状态
4. **错误处理** - 使用框架提供的错误类型，确保一致的错误处理
5. **分层架构** - 遵循模型-服务-控制器的分层架构

## 高级功能

### 中间件系统

mp Framework 支持中间件，用于处理横切关注点：

```rust
// 自定义中间件：请求计时
async fn timing_middleware(
    req: HttpRequest,
    ctx: Context,
    next: Next,
) -> Result<HttpResponse<serde_json::Value>> {
    // 记录开始时间
    let start = std::time::Instant::now();

    // 调用下一个中间件
    let response = next(req.clone(), ctx).await?;

    // 计算耗时
    let duration = start.elapsed();
    println!("[TIMING] {} took {:?}", req.path, duration);

    Ok(response)
}
```

### 批量操作

支持批量实体操作，提高性能：

```rust
// 批量保存实体
fn batch_save<T>(&mut self, entities: &[T]) -> Result<()> 
where 
    T: Model + StateSerializable + Identifiable + Serialize
{
    for entity in entities {
        self.save(entity)?;
    }
    Ok(())
}
```

## 结论

mp Framework 通过提供类似 Web2 的开发体验，同时保留区块链的核心特性，大大降低了区块链开发的学习曲线和复杂性。其自动状态跟踪和分层架构使开发者能够专注于业务逻辑，而不是底层细节。
