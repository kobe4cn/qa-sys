# qa-sys

`qa-sys` 是一个用于练习 Rust 2024、Axum、Tonic/gRPC、PostgreSQL、Redis 和
Apache Pulsar 的问答系统 sample 项目。它展示了 HTTP Gateway、gRPC 服务、关系型
存储、缓存与消息队列如何协同工作。

> [!WARNING]
> 本项目仅供学习和本地实验，不具备生产环境所需的安全性、运维能力与兼容性，请勿直接部署
> 到生产环境。

## 架构

```text
HTTP Client
    │
    ▼
Gateway :8090 ── gRPC ──▶ qa-svc :50051
    │                       ├── PostgreSQL :5432
    │                       ├── Redis      :6379
    │                       └── Pulsar     :6650
    └── Metrics :1338           └── Metrics :2338
```

| Workspace 模块 | 职责 |
| --- | --- |
| `crates/gateway` | 对外提供 HTTP API，执行输入校验、Bearer Token 鉴权并调用 gRPC 服务 |
| `crates/qa-svc` | 实现用户、问题、回答与点赞等核心业务，通过 gRPC 暴露能力 |
| `crates/pb` | 提供由 Protocol Buffers 生成的共享 gRPC 类型 |
| `crates/core` | 提供配置、加解密、数据库、Redis、Pulsar、指标与优雅退出等基础能力 |
| `apps/server` | Workspace 中保留的应用示例 crate |

完整 gRPC 合约见 [`proto/qa.proto`](proto/qa.proto)，HTTP 路由以
[`crates/gateway/src/router.rs`](crates/gateway/src/router.rs) 为准。

## 前置要求

- Apple Silicon Mac 与受支持的 macOS，使用
  [Apple `container`](https://github.com/apple/container) 1.1 或更新版本；或
- [Podman](https://podman.io/docs/installation) 5 或更新版本；
- `make`；
- `curl`；
- [`jq`](https://jqlang.org/download/)（用于从登录响应提取 Token）。

项目通过 [`rust-toolchain.toml`](rust-toolchain.toml) 固定 Rust 工具链，首次执行
Cargo 命令时 `rustup` 会按需安装。

本地依赖使用以下固定镜像：

| 依赖 | 镜像 |
| --- | --- |
| PostgreSQL | `postgres:18.4` |
| Redis | `redis:8.8.1-alpine` |
| Pulsar | `apachepulsar/pulsar:4.2.3` |

容器密码只从环境变量读取，不在 Makefile 中提供默认值。以下是与示例
`app.yaml` 一致的本地学习凭据；如需覆盖，也要同步更新 `app.yaml` 中的连接地址：

```bash
export POSTGRES_PASSWORD=postgres
export REDIS_PASSWORD=redis
```

密码必须为 1–128 字节，只能包含 ASCII 字母、数字和
`. _ ~ @ % + = , : /`。不符合约束时 Make 目标会在创建容器前拒绝执行。

## 快速开始：Apple `container`

Apple `container` 只支持 Apple Silicon。安装方法和受支持的 macOS 版本以
[官方安装说明](https://github.com/apple/container#initial-install) 为准。

先启动 `container` 系统服务：

```bash
container system start
```

如需构建项目镜像，使用 Makefile 提供的兼容入口：

```bash
make images-build-apple
```

该命令依次生成 `qa-project-dev:v1.0`、`qa-svc:latest` 和
`qa-gateway:latest`。Apple `container` 1.1.0 在部分目录（包括当前 SynologyDrive
工作区）递归归档 `COPY . .` 构建上下文时可能遗漏嵌套文件或报告
`invalid tar header`。Makefile 会创建不含 `.env`、`.git`、`target` 和 macOS
AppleDouble 元数据的临时单文件上下文，构建结束后自动删除；标准 Dockerfile
仍可直接供 Podman 或 Docker 使用。

也可以只构建所需镜像：

```bash
make image-build-dev-apple
make image-build-qa-svc-apple
make image-build-gateway-apple
```

镜像标签、目标平台和构建器内存可覆盖：

```bash
make images-build-apple \
  APPLE_BUILD_PLATFORM=linux/arm64 \
  DEV_IMAGE=qa-project-dev:local \
  QA_SVC_IMAGE=qa-svc:local \
  GATEWAY_IMAGE=qa-gateway:local \
  APPLE_BUILD_MEMORY=8g
```

`APPLE_BUILD_PLATFORM` 默认为 `linux/arm64`。构建 AMD64 / x86_64 镜像时，应同时使用
架构专用标签，避免覆盖本地 ARM64 镜像：

```bash
make images-build-apple \
  APPLE_BUILD_PLATFORM=linux/amd64 \
  DEV_IMAGE=qa-project-dev:v1.0-amd64 \
  QA_SVC_IMAGE=qa-svc:amd64 \
  GATEWAY_IMAGE=qa-gateway:amd64
```

每次调用生成一个架构的镜像，不会自动创建同时包含 ARM64 和 AMD64 的多架构
manifest。

然后启动依赖并执行数据库迁移：

```bash
make deps-up-apple
make db-migrate-apple
```

也可以合并为一条命令：

```bash
make deps-up-apple && make db-migrate-apple
```

`db-migrate-apple` 会等待 PostgreSQL、Redis 和 Pulsar 全部就绪。第一次执行会创建
业务表；再次执行会识别已完成的迁移并安全跳过。

## 快速开始：Podman

macOS 上的 Podman 需要一个正在运行的 Podman machine。项目只检查现有 machine，不会
自动创建、启动或修改它：

```bash
podman machine init  # 仅首次使用 Podman 时需要
podman machine start
```

启动依赖并迁移数据库：

```bash
make deps-up-podman
make db-migrate-podman
```

也可以合并为：

```bash
make deps-up-podman && make db-migrate-podman
```

Podman machine 的安装与管理细节见
[Podman 官方文档](https://docs.podman.io/en/latest/markdown/podman-machine.1.html)。

## 启动应用

`qa-svc` 必须先于 Gateway 启动。两个进程都保持在前台，便于观察日志和使用
`Ctrl+C` 停止。

终端 1：

```bash
make run-service
```

该命令通过 `APP_CONFIG` 读取根目录 [`app.yaml`](app.yaml)。

确认 `qa-svc` 已监听 `50051` 后，在终端 2 执行：

```bash
make run-gateway
```

Gateway 通过 `QA_CONFIG_DIR` 读取
[`crates/gateway/app-gw.yaml`](crates/gateway/app-gw.yaml)，默认监听 `8090`。

浏览器或第三个终端可先验证公开入口：

```bash
curl --fail-with-body http://127.0.0.1:8090/api/hello
```

## HTTP 业务验证

以下流程验证 Gateway、`qa-svc`、PostgreSQL、Redis 和 Pulsar 的完整调用链。

### 1. 注册

```bash
curl --fail-with-body \
  -H 'content-type: application/json' \
  -d '{"username":"qa_demo","password":"sample123","email":"qa_demo@example.com","phone":"021234567"}' \
  http://127.0.0.1:8090/api/user/register
```

命名卷会保留用户数据，因此重复注册 `qa_demo` 失败是预期行为。可以换一个用户名，或在确认
不再需要本地数据后执行对应容器引擎的 `deps-reset-*`。

### 2. 登录并提取 Token

```bash
TOKEN="$(curl --fail-with-body --silent \
  -H 'content-type: application/json' \
  -d '{"username":"qa_demo","password":"sample123"}' \
  http://127.0.0.1:8090/api/user/login | jq -r '.token')"

test -n "${TOKEN}" && test "${TOKEN}" != "null"
```

### 3. 创建问题

```bash
curl --fail-with-body \
  -H 'content-type: application/json' \
  -H "authorization: Bearer ${TOKEN}" \
  -d '{"title":"第一个问题","content":"qa-sys 是如何工作的？","username":"qa_demo"}' \
  http://127.0.0.1:8090/api/question/add
```

### 4. 查询最新问题

```bash
curl --fail-with-body \
  -H "authorization: Bearer ${TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"last_id":0,"limit":10}' \
  http://127.0.0.1:8090/api/question/find_latest
```

创建问题和查询最新问题都是受保护路由，必须携带登录得到的 Bearer Token。

## 常用 Makefile 目标

| 目标 | 作用 |
| --- | --- |
| `images-build-apple` | 使用 Apple `container` 兼容上下文构建全部三个项目镜像 |
| `image-build-dev-apple` | 构建包含 Rust、Go、Node 和 Python 的开发镜像 |
| `image-build-qa-svc-apple` | 构建开发镜像和 `qa-svc` 运行镜像 |
| `image-build-gateway-apple` | 构建开发镜像和 Gateway 运行镜像 |
| `deps-up-apple` | 使用 Apple `container` 创建或重建本项目依赖，保留已有命名卷 |
| `deps-status-apple` | 等待 Apple `container` 中的三个依赖就绪 |
| `db-migrate-apple` | 检查迁移状态并通过 Apple `container` 执行迁移 |
| `deps-down-apple` | 删除 Apple `container` 项目容器，保留数据卷 |
| `deps-reset-apple` | 删除 Apple `container` 项目容器和数据卷 |
| `deps-up-podman` | 使用 Podman 创建或重建本项目依赖，保留已有命名卷 |
| `deps-status-podman` | 等待 Podman 中的三个依赖就绪 |
| `db-migrate-podman` | 检查迁移状态并通过 Podman 执行迁移 |
| `deps-down-podman` | 删除 Podman 项目容器，保留数据卷 |
| `deps-reset-podman` | 删除 Podman 项目容器和数据卷 |
| `run-service` | 前台启动 `qa-svc` |
| `run-gateway` | 前台启动 HTTP Gateway |
| `test-unit` | 运行 workspace 的单元测试 |
| `test-integration` | 顺序运行依赖、gRPC、Gateway 和端到端集成测试 |
| `test-all` | 运行单元测试与全部集成测试 |

容器名、卷名和端口可通过 Make 变量覆盖。例如：

```bash
make deps-up-apple \
  DEV_PREFIX=qa-sys-sandbox \
  POSTGRES_PORT=15432 \
  REDIS_PORT=16379 \
  PULSAR_PORT=16650 \
  PULSAR_HTTP_PORT=18080
```

覆盖依赖端口后，应用配置中的连接地址也必须同步调整，才能让应用连接到该隔离环境。

## 配置与端口

| 组件 | 默认端口 | 配置来源 | 用途 |
| --- | ---: | --- | --- |
| Gateway | `8090` | `crates/gateway/app-gw.yaml` | HTTP API |
| Gateway metrics | `1338` | `crates/gateway/app-gw.yaml` | Prometheus metrics |
| `qa-svc` | `50051` | `app.yaml` | gRPC API |
| `qa-svc` metrics | `2338` | `app.yaml` | Prometheus metrics |
| PostgreSQL | `5432` | `app.yaml` / Makefile | `qa_sys` 数据库 |
| Redis | `6379` | `app.yaml` / Makefile | 登录会话与读取计数 |
| Pulsar | `6650` | `app.yaml` / Makefile | 消息协议 |
| Pulsar admin | `8080` | Makefile | HTTP 管理接口 |

示例本地凭据仅用于学习：

- PostgreSQL：用户 `postgres`，密码 `postgres`，数据库 `qa_sys`；
- Redis：密码 `redis`。

PostgreSQL 用户和数据库还可通过 `POSTGRES_USER`、`POSTGRES_DATABASE` Make
变量覆盖。

## 测试

依赖集成测试会连接 `app.yaml` 中的本地服务。先启动依赖并完成迁移，再选择需要的测试层级：

```bash
make test-unit
make test-integration
make test-all
```

也可以单独运行 PostgreSQL、Redis、Pulsar、gRPC、Gateway 或端到端测试：

```bash
make test-postgres
make test-redis
make test-pulsar
make test-grpc
make test-gateway-integration
make test-e2e
```

## 数据持久化与重置

PostgreSQL、Redis、Pulsar 数据和 Pulsar 日志默认存储在带 `qa-sys` 前缀的命名卷中。

普通停止只删除容器，数据会保留：

```bash
make deps-down-apple
# 或
make deps-down-podman
```

再次执行对应的 `deps-up-*` 后可以继续使用原有数据。

> [!CAUTION]
> 下列 reset 命令会永久删除对应引擎中带当前 `DEV_PREFIX` 的项目数据卷。请先确认没有需要
> 保留的数据。

```bash
make deps-reset-apple
# 或
make deps-reset-podman
```

这些目标只操作由 `DEV_PREFIX` 派生的精确容器名和卷名，不执行全局 prune。

## 常见问题

### Apple `container` 系统未运行

```bash
container system status
container system start
```

### Podman 无法连接

项目不会自动改变 Podman machine。先检查并按需启动现有 machine：

```bash
podman machine list
podman machine start
podman info
```

### 端口已被占用

检查相应端口的占用进程，或通过 Make 变量改用其他依赖端口：

```bash
lsof -nP -iTCP:5432 -sTCP:LISTEN
```

如果修改依赖端口，请同步修改应用配置。

### 依赖容器已启动，但应用仍无法连接

容器启动不代表服务已经就绪。执行对应引擎的状态或迁移目标：

```bash
make deps-status-apple
# 或
make deps-status-podman
```

Pulsar 通常比 PostgreSQL 和 Redis 需要更长启动时间。

### 数据库处于部分迁移状态

迁移目标只接受四张核心表全部不存在或全部存在。如果提示部分迁移，请先检查数据库；确认
不需要保留数据后，再使用对应的 `deps-reset-*` 重建本地环境。

## 已知安全限制

这些限制是本项目只能用于学习的主要原因：

- 用户密码当前使用 MD5，不能满足生产环境的密码存储要求；
- Token 加密使用 AES-CBC 和固定 IV；
- 示例 AES 密钥、IV、PostgreSQL 密码与 Redis 密码保存在仓库配置中；
- Apple `container` 的 Redis、Pulsar 以及 Podman 的 Pulsar 为兼容本地命名卷而以容器内
  root 用户运行；
- HTTP 和本地依赖连接未提供生产级 TLS；
- 没有完整的密钥轮换、Secret Manager、限流、审计、高可用和生产部署方案。

如果希望将相关思路用于真实系统，应重新设计身份认证、密码哈希、加密、Secret 管理、
网络边界、授权、可观测性和运维流程，而不是直接复用本项目配置。
