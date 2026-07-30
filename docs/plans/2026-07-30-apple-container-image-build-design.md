# Apple container 镜像构建兼容设计

## 背景

Apple `container` 1.1.0 在当前环境中归档递归构建上下文时存在异常：

- SynologyDrive 工作区可能在 `load build context` 阶段报告
  `archive/tar: invalid tar header`；
- 即使将源码复制到 `/tmp`，`COPY . .` 也只会传递顶层文件，嵌套目录中的文件被静默遗漏。

开发镜像不复制项目源码，因此能够正常构建；`qa-svc` 和 Gateway 的多阶段镜像需要完整
Rust workspace，不能直接使用这一归档行为。

## 目标

- 使用 Apple `container` 构建开发、`qa-svc` 和 Gateway 镜像。
- 保持现有标准 Dockerfile 可供 Podman、Docker 和其他 OCI 构建器使用。
- 不逐条维护 Rust 源文件清单。
- 不把 `.env`、`.git` 或 `target` 写入镜像构建上下文，也不在工作树中遗留临时归档。
- 构建成功、失败或被中断后都清理临时目录。

## 方案选择

采用 Makefile 临时单文件上下文：

1. `mktemp -d` 创建独立构建目录；
2. 使用 BSD tar 打包工作树，同时禁用 macOS AppleDouble/xattr 元数据并排除敏感、生成文件；
3. 使用 `awk` 将标准 Dockerfile 的 `COPY . .` 临时转换为复制并解包 `source.tar`；
4. 将转换后的 Dockerfile 写入临时上下文并交给 Apple `container build`；
5. shell trap 始终删除临时目录。

该方案以现有 Dockerfile 为唯一运行镜像定义，避免维护 Apple 专用 Dockerfile 副本。
应用构建通过 `DEV_IMAGE` build arg 使用同一次 Make 调用生成的开发镜像标签。
聚合目标只生成一次源码归档，`qa-svc` 与 Gateway 构建共享该上下文，使第二个应用镜像
能够复用完整 workspace 的 release 构建缓存。

## 未采用方案

- **逐文件 `COPY`**：能够绕过归档缺陷，但任何新增 Rust 文件都可能被漏掉，维护成本高。
- **提交固定 `source.tar`**：会产生过期构建输入，并增加误提交密钥或本地文件的风险。
- **复制一套 Apple 专用 Dockerfile**：会重复运行镜像配置，后续容易与标准 Dockerfile 漂移。

## Makefile 接口

- `image-build-dev-apple`：构建 `qa-project-dev:v1.0`。
- `image-build-qa-svc-apple`：构建开发镜像及 `qa-svc:latest`。
- `image-build-gateway-apple`：构建开发镜像及 `qa-gateway:latest`。
- `images-build-apple`：构建全部三个镜像。

镜像名称、目标平台与构建内存可通过 Make 变量覆盖。目标平台默认为
`linux/arm64`，也支持通过 `APPLE_BUILD_PLATFORM=linux/amd64` 生成单架构的
AMD64 镜像；不同架构应使用不同标签。

## 验证

- 先确认目标不存在，得到预期 RED 结果。
- 对全部目标执行 `make -n`，验证命令展开和清理 trap。
- 实际执行 `make images-build-apple`。
- 使用架构专用标签实际执行一次 `linux/amd64` 构建并检查镜像平台。
- 使用一次性容器验证二进制、配置环境变量和保留工具。
- 检查 Git 状态，确认没有生成的 tar 或临时构建目录进入工作区。
