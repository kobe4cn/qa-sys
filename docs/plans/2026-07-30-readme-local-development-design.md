# README 本地开发体验设计

## 背景

`qa-sys` 是用于练习 Rust Web、gRPC、PostgreSQL、Redis 和 Pulsar 的示例项目。根目录
`README.md` 当前为空，使用者无法仅凭仓库内容完成依赖启动、数据库初始化、应用运行和
基本功能验证。

本次设计面向希望在本地启动项目的学习者，不以生产部署为目标。

## 目标

- 用中文说明项目定位、架构和模块职责。
- 支持 Apple `container` 和 Podman 两种本地容器运行方式。
- 通过 Makefile 提供可发现、可重复执行的依赖管理命令。
- 默认持久化 PostgreSQL、Redis 和 Pulsar 数据。
- 提供显式的数据重置命令，避免意外删除练习数据。
- 使用两个前台终端分别运行 gRPC 服务与 HTTP Gateway。
- 提供一条可复制的 HTTP 业务验证链路。
- 明确列出该示例项目不适合生产环境的安全限制。

## 非目标

- 不提供 Kubernetes、Helm 或生产部署方案。
- 不引入 Docker Compose 或 Podman Compose。
- 不把应用进程放入后台，也不在 Makefile 中维护 PID。
- 不完整复制所有 HTTP 或 gRPC 接口文档。
- 不在本次工作中修复项目已有的生产安全限制。

## 方案选择

Makefile 直接封装 Apple `container` 和 Podman 两套 CLI。两套实现共用镜像版本、端口、
容器名和卷名变量，不新增 Compose 文件。

该方案的优点是：

- 与 Apple `container` 的原生工作方式一致；
- Podman 使用者无需额外安装 Compose 工具；
- 所有本地自动化入口都集中在现有 Makefile；
- 两套运行环境的差异保持可见，排障更直接。

## 依赖拓扑

| 依赖 | 镜像 | 主机端口 | 持久化位置 |
| --- | --- | --- | --- |
| PostgreSQL | `postgres:18.4` | `5432` | `/var/lib/postgresql` |
| Redis | `redis:8.8.1-alpine` | `6379` | `/data` |
| Pulsar | `apachepulsar/pulsar:4.2.3` | `6650`、`8080` | `/pulsar/data`、`/pulsar/logs` |

PostgreSQL 创建 `qa_sys` 数据库，账号和密码均为 `postgres`。Redis 密码为 `redis`。
这些值与现有 `app.yaml` 保持一致，仅用于本地开发。

## Makefile 接口

面向使用者提供以下目标：

- `deps-up-apple` / `deps-up-podman`
- `deps-status-apple` / `deps-status-podman`
- `db-migrate-apple` / `db-migrate-podman`
- `deps-down-apple` / `deps-down-podman`
- `deps-reset-apple` / `deps-reset-podman`
- `run-service`
- `run-gateway`

`deps-up-*` 重建项目专用容器但保留命名卷。`deps-down-*` 删除容器但保留数据。
`deps-reset-*` 删除容器和项目专用卷，并在执行前明确说明数据会被清空。

Apple `container` 运行 Pulsar 时分配 2 GiB 内存，避免其默认 1 GiB 虚拟机内存不足。

## 数据库迁移

`db-migrate-*` 使用容器内的 `psql` 执行现有
`migrations/20260725064428_db.sql`。

执行前检查 `questions`、`answers`、`users` 和 `users_votes`：

- 四张表全部存在：打印已迁移并成功退出；
- 四张表全部不存在：执行迁移；
- 仅存在部分表：失败并提示使用者检查数据库或执行 reset。

迁移使用 `ON_ERROR_STOP`，任何 SQL 错误都会让 Make 目标失败。

## 应用启动与数据流

使用者在两个终端中运行：

1. `make run-service`：读取根目录 `app.yaml`，启动 `qa-svc` gRPC 服务；
2. `make run-gateway`：读取 `crates/gateway/app-gw.yaml`，启动 HTTP Gateway。

请求链路为：

```text
HTTP 客户端 → Gateway :8090 → qa-svc gRPC :50051
                               ├─ PostgreSQL :5432
                               ├─ Redis :6379
                               └─ Pulsar :6650
```

README 使用注册、登录、Bearer Token 创建问题和查询问题列表验证整条链路。

## README 信息架构

1. 项目定位与非生产警告
2. 架构和 workspace 模块
3. 前置要求
4. Apple `container` 快速开始
5. Podman 快速开始
6. 启动服务
7. HTTP 业务验证
8. 常用 Makefile 目标
9. 配置与端口
10. 测试
11. 数据持久化和重置
12. 常见问题
13. 已知安全限制

## 错误处理

- 未安装所选容器 CLI 时立即失败并给出提示。
- 依赖端口被占用时保留底层 CLI 错误，README 提供排查命令。
- 依赖未就绪时状态检查返回失败，不静默继续。
- 数据库处于部分迁移状态时拒绝自动覆盖。
- reset 只操作带 `qa-sys` 前缀的容器和卷。

## 验证

- 对两套 Makefile 目标执行 dry-run，检查命令展开。
- 在当前 Apple `container` 环境实际验证依赖启动、迁移和状态检查。
- 若 Podman 在本机可用，则执行同样验证；否则进行命令级验证并明确记录。
- 启动 gRPC 服务和 Gateway，执行 README 中的 HTTP 流程。
- 修改仅涉及 README 和 Makefile 时，不运行无关的 Rust 全量门禁；检查 Markdown、
  Makefile dry-run、链接和实际启动流程。

## 已知安全限制

README 必须明确说明：

- 用户密码当前使用 MD5；
- AES-CBC 使用固定 IV；
- 示例密钥和本地服务密码保存在配置文件中；
- 项目没有正式的密钥轮换、限流和生产部署方案；
- 该项目仅用于学习，不应直接部署到生产环境。
