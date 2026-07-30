# README Local Development Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite the Chinese README and extend the Makefile so a learner can start the complete
qa-sys stack with Apple `container` or Podman, initialize the database, run both application
processes, and verify an HTTP workflow.

**Architecture:** Keep infrastructure automation in the existing Makefile. Both container engines
share pinned image, port, container, and volume variables but have explicit public targets so no
hidden engine state is required. The README leads with one quick-start path, then documents the
alternative engine, application data flow, verification commands, tests, reset behavior, and
non-production limitations.

**Tech Stack:** GNU/BSD Make, Apple `container` 1.1+, Podman 5+, PostgreSQL 18, Redis 8, Apache
Pulsar 4, Rust/Cargo, curl, jq, Markdown.

---

### Task 1: Add container lifecycle automation

**Files:**
- Modify: `Makefile`

**Step 1: Verify the targets do not exist**

Run:

```bash
make -n deps-up-apple
make -n deps-up-podman
```

Expected: both commands fail with `No rule to make target`.

**Step 2: Add shared development variables**

Add these variables after `APP_CONFIG_PATH`:

```make
DEV_PREFIX ?= qa-sys

POSTGRES_IMAGE ?= docker.io/library/postgres:18.4
REDIS_IMAGE ?= docker.io/library/redis:8.8.1-alpine
PULSAR_IMAGE ?= docker.io/apachepulsar/pulsar:4.2.3

POSTGRES_PORT ?= 5432
REDIS_PORT ?= 6379
PULSAR_PORT ?= 6650
PULSAR_HTTP_PORT ?= 8080

POSTGRES_CONTAINER := $(DEV_PREFIX)-postgres
REDIS_CONTAINER := $(DEV_PREFIX)-redis
PULSAR_CONTAINER := $(DEV_PREFIX)-pulsar

POSTGRES_VOLUME := $(DEV_PREFIX)-postgres-data
REDIS_VOLUME := $(DEV_PREFIX)-redis-data
PULSAR_DATA_VOLUME := $(DEV_PREFIX)-pulsar-data
PULSAR_LOG_VOLUME := $(DEV_PREFIX)-pulsar-logs
```

The configurable prefix and ports permit isolated validation without touching a learner's existing
containers.

**Step 3: Add Apple `container` lifecycle targets**

Implement:

```make
check-apple-container:
	@command -v container >/dev/null 2>&1 || { \
		echo "Apple container CLI is not installed"; \
		exit 1; \
	}
	@container system status >/dev/null

deps-up-apple: check-apple-container
	-@container delete --force $(POSTGRES_CONTAINER) $(REDIS_CONTAINER) $(PULSAR_CONTAINER)
	@container volume inspect $(POSTGRES_VOLUME) >/dev/null 2>&1 || \
		container volume create $(POSTGRES_VOLUME) >/dev/null
	@container volume inspect $(REDIS_VOLUME) >/dev/null 2>&1 || \
		container volume create $(REDIS_VOLUME) >/dev/null
	@container volume inspect $(PULSAR_DATA_VOLUME) >/dev/null 2>&1 || \
		container volume create $(PULSAR_DATA_VOLUME) >/dev/null
	@container volume inspect $(PULSAR_LOG_VOLUME) >/dev/null 2>&1 || \
		container volume create $(PULSAR_LOG_VOLUME) >/dev/null
	@container run -d --name $(POSTGRES_CONTAINER) \
		-p $(POSTGRES_PORT):5432 \
		-e POSTGRES_USER=postgres \
		-e POSTGRES_PASSWORD=postgres \
		-e POSTGRES_DB=qa_sys \
		-v $(POSTGRES_VOLUME):/var/lib/postgresql \
		$(POSTGRES_IMAGE)
	@container run -d --name $(REDIS_CONTAINER) \
		-p $(REDIS_PORT):6379 \
		-v $(REDIS_VOLUME):/data \
		$(REDIS_IMAGE) \
		redis-server --appendonly yes --requirepass redis
	@container run -d --name $(PULSAR_CONTAINER) \
		--memory 2g \
		-p $(PULSAR_PORT):6650 \
		-p $(PULSAR_HTTP_PORT):8080 \
		-v $(PULSAR_DATA_VOLUME):/pulsar/data \
		-v $(PULSAR_LOG_VOLUME):/pulsar/logs \
		$(PULSAR_IMAGE) \
		bin/pulsar standalone

deps-down-apple: check-apple-container
	-@container delete --force $(POSTGRES_CONTAINER) $(REDIS_CONTAINER) $(PULSAR_CONTAINER)

deps-reset-apple: deps-down-apple
	@echo "Deleting qa-sys Apple container volumes and all local dependency data"
	-@container volume delete \
		$(POSTGRES_VOLUME) \
		$(REDIS_VOLUME) \
		$(PULSAR_DATA_VOLUME) \
		$(PULSAR_LOG_VOLUME)
```

Only exact project-prefixed container and volume names may be deleted. Do not use `--all` or prune
commands.

**Step 4: Add Podman lifecycle targets**

Implement the equivalent Podman targets:

```make
check-podman:
	@command -v podman >/dev/null 2>&1 || { \
		echo "Podman is not installed"; \
		exit 1; \
	}
	@podman info >/dev/null

deps-up-podman: check-podman
	-@podman rm --force $(POSTGRES_CONTAINER) $(REDIS_CONTAINER) $(PULSAR_CONTAINER)
	@podman volume create $(POSTGRES_VOLUME) >/dev/null
	@podman volume create $(REDIS_VOLUME) >/dev/null
	@podman volume create $(PULSAR_DATA_VOLUME) >/dev/null
	@podman volume create $(PULSAR_LOG_VOLUME) >/dev/null
	@podman run -d --name $(POSTGRES_CONTAINER) \
		-p $(POSTGRES_PORT):5432 \
		-e POSTGRES_USER=postgres \
		-e POSTGRES_PASSWORD=postgres \
		-e POSTGRES_DB=qa_sys \
		-v $(POSTGRES_VOLUME):/var/lib/postgresql \
		$(POSTGRES_IMAGE)
	@podman run -d --name $(REDIS_CONTAINER) \
		-p $(REDIS_PORT):6379 \
		-v $(REDIS_VOLUME):/data \
		$(REDIS_IMAGE) \
		redis-server --appendonly yes --requirepass redis
	@podman run -d --name $(PULSAR_CONTAINER) \
		-p $(PULSAR_PORT):6650 \
		-p $(PULSAR_HTTP_PORT):8080 \
		-v $(PULSAR_DATA_VOLUME):/pulsar/data \
		-v $(PULSAR_LOG_VOLUME):/pulsar/logs \
		$(PULSAR_IMAGE) \
		bin/pulsar standalone

deps-down-podman: check-podman
	-@podman rm --force $(POSTGRES_CONTAINER) $(REDIS_CONTAINER) $(PULSAR_CONTAINER)

deps-reset-podman: deps-down-podman
	@echo "Deleting qa-sys Podman volumes and all local dependency data"
	-@podman volume rm \
		$(POSTGRES_VOLUME) \
		$(REDIS_VOLUME) \
		$(PULSAR_DATA_VOLUME) \
		$(PULSAR_LOG_VOLUME)
```

**Step 5: Verify command expansion**

Run:

```bash
make -n deps-up-apple
make -n deps-up-podman
make -n deps-down-apple
make -n deps-down-podman
make -n deps-reset-apple
make -n deps-reset-podman
```

Expected: every target expands successfully; every destructive command contains only values derived
from `DEV_PREFIX`.

**Step 6: Commit**

```bash
git add Makefile
git commit -m "build: add local dependency lifecycle targets"
```

### Task 2: Add readiness and migration automation

**Files:**
- Modify: `Makefile`
- Read: `migrations/20260725064428_db.sql`

**Step 1: Verify status and migration targets do not exist**

Run:

```bash
make -n deps-status-apple
make -n db-migrate-apple
make -n deps-status-podman
make -n db-migrate-podman
```

Expected: all targets fail with `No rule to make target`.

**Step 2: Add readiness targets**

For each engine, implement bounded retry loops for:

```text
PostgreSQL: pg_isready -U postgres -d qa_sys
Redis:      redis-cli -a redis --no-auth-warning ping
Pulsar:     bin/pulsar-admin clusters list
```

Each service gets at most 60 attempts with a one-second interval. If a service never becomes ready,
print its name and return a non-zero exit status. Do not silently ignore readiness failures.

`deps-status-apple` depends on `check-apple-container`; `deps-status-podman` depends on
`check-podman`.

**Step 3: Add migration state validation**

Use this SQL for both engines:

```sql
SELECT count(*)
FROM information_schema.tables
WHERE table_schema = 'public'
  AND table_name IN ('questions', 'answers', 'users', 'users_votes');
```

The recipe must:

1. run the query through the selected engine's `exec`;
2. trim `psql` formatting from the result;
3. skip when the result is `4`;
4. execute `psql --set ON_ERROR_STOP=1` with stdin from
   `migrations/20260725064428_db.sql` when the result is `0`;
5. fail with a partial-migration message for `1`, `2`, `3`, or unexpected output.

Public targets:

```make
db-migrate-apple: deps-status-apple
db-migrate-podman: deps-status-podman
```

**Step 4: Verify command expansion and shell syntax**

Run:

```bash
make -n deps-status-apple
make -n deps-status-podman
make -n db-migrate-apple
make -n db-migrate-podman
```

Expected: all targets expand successfully, reference the correct container names, and use
`ON_ERROR_STOP`.

**Step 5: Commit**

```bash
git add Makefile
git commit -m "build: add dependency readiness and migration targets"
```

### Task 3: Add foreground application targets

**Files:**
- Modify: `Makefile`

**Step 1: Verify application targets do not exist**

Run:

```bash
make -n run-service
make -n run-gateway
```

Expected: both targets fail with `No rule to make target`.

**Step 2: Add the targets**

Implement:

```make
run-service:
	@APP_CONFIG="$(APP_CONFIG_PATH)" cargo run -p qa-svc

run-gateway:
	@QA_CONFIG_DIR="$(CURDIR)/crates/gateway" cargo run -p gateway
```

Do not background either process.

**Step 3: Update `.PHONY`**

Add every new public and validation target from Tasks 1–3 to `.PHONY`.

**Step 4: Verify command expansion**

Run:

```bash
make -n run-service
make -n run-gateway
```

Expected: the service uses the absolute root `app.yaml`; the Gateway uses
`crates/gateway/app-gw.yaml` through `QA_CONFIG_DIR`.

**Step 5: Commit**

```bash
git add Makefile
git commit -m "build: add foreground application targets"
```

### Task 4: Rewrite the root README

**Files:**
- Modify: `README.md`
- Read: `app.yaml`
- Read: `crates/gateway/app-gw.yaml`
- Read: `crates/gateway/src/router.rs`
- Read: `proto/qa.proto`

**Step 1: Create content acceptance checks**

Before writing, run:

```bash
rg -n "仅供学习|deps-up-apple|deps-up-podman|run-service|run-gateway|Bearer|已知限制" README.md
```

Expected: no matches because the README is empty.

**Step 2: Write the README**

Use the confirmed structure:

```text
# qa-sys
项目定位和非生产警告
架构图
workspace 模块表
前置要求
Apple container 快速开始
Podman 快速开始
两个终端启动应用
HTTP 业务验证
Makefile 目标表
端口和配置表
测试
持久化与重置
常见问题
已知安全限制
```

Requirements:

- Chinese prose; retain English command and identifier names.
- State that Apple `container` requires Apple silicon and a supported macOS release.
- Link to the official Apple `container` and Podman installation documentation.
- Use `make deps-up-apple && make db-migrate-apple` as the primary quick start.
- Show the Podman equivalents immediately afterward.
- Explain that `deps-down-*` preserves data and `deps-reset-*` destroys data.
- Explain that `qa-svc` must start before `gateway`.
- Document ports 8090, 50051, 2338, 1338, 5432, 6379, 6650, and 8080.
- Link to `proto/qa.proto` for the complete gRPC contract.
- Link to `crates/gateway/src/router.rs` for the HTTP route source of truth.

Use this HTTP workflow:

```bash
curl --fail-with-body \
  -H 'content-type: application/json' \
  -d '{"username":"qa_demo","password":"sample123","email":"qa_demo@example.com","phone":"021234567"}' \
  http://127.0.0.1:8090/api/user/register

TOKEN="$(curl --fail-with-body --silent \
  -H 'content-type: application/json' \
  -d '{"username":"qa_demo","password":"sample123"}' \
  http://127.0.0.1:8090/api/user/login | jq -r '.token')"

curl --fail-with-body \
  -H 'content-type: application/json' \
  -H "authorization: Bearer ${TOKEN}" \
  -d '{"title":"第一个问题","content":"qa-sys 是如何工作的？","username":"qa_demo"}' \
  http://127.0.0.1:8090/api/question/add

curl --fail-with-body \
  -H "authorization: Bearer ${TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"last_id":0,"limit":10}' \
  http://127.0.0.1:8090/api/question/find_latest
```

Mention that a duplicate registration is expected to fail and the learner should choose a new
username or reset local data.

**Step 3: Run content acceptance checks**

Run:

```bash
rg -n "仅供学习|deps-up-apple|deps-up-podman|run-service|run-gateway|Bearer|已知安全限制" README.md
rg -n "proto/qa.proto|crates/gateway/src/router.rs|app.yaml|app-gw.yaml" README.md
```

Expected: all required topics and links are present.

**Step 4: Proofread Markdown**

Check:

- every heading has content;
- all fenced code blocks are closed;
- local links resolve;
- commands use the documented target names;
- no claim says the project is production-ready;
- no secret is presented as a production credential.

**Step 5: Commit**

```bash
git add README.md
git commit -m "docs: add local development guide"
```

### Task 5: Validate both container engines safely

**Files:**
- Verify: `Makefile`
- Verify: `README.md`

**Step 1: Validate Apple `container` in an isolated namespace**

Use non-default names and ports:

```bash
make deps-up-apple \
  DEV_PREFIX=qa-sys-readme-test \
  POSTGRES_PORT=15432 \
  REDIS_PORT=16379 \
  PULSAR_PORT=16650 \
  PULSAR_HTTP_PORT=18080
```

Run the matching `deps-status-apple` and `db-migrate-apple` commands with the same overrides.

Expected: all services become ready and the first migration succeeds.

Run `db-migrate-apple` a second time.

Expected: it reports that all four tables already exist and exits successfully.

**Step 2: Remove only the isolated Apple validation resources**

Run `deps-reset-apple` with the exact same `DEV_PREFIX` and port overrides.

Expected: only `qa-sys-readme-test-*` containers and volumes are removed.

**Step 3: Validate Podman if its machine is available**

Run the equivalent isolated commands with:

```text
DEV_PREFIX=qa-sys-readme-podman-test
POSTGRES_PORT=25432
REDIS_PORT=26379
PULSAR_PORT=26650
PULSAR_HTTP_PORT=28080
```

If Podman is installed but its machine is not initialized or running, do not mutate global Podman
machine state automatically. Report that actual Podman runtime validation was skipped and retain
the successful dry-run evidence.

**Step 4: Validate the application workflow**

Against dependencies on the default ports:

1. start `make run-service` in one foreground terminal;
2. start `make run-gateway` in a second foreground terminal;
3. execute the four README HTTP commands;
4. confirm registration, login, authenticated question creation, and latest-question lookup;
5. stop both foreground processes with Ctrl-C.

Use a unique username if persistent data already contains `qa_demo`.

**Step 5: Run final documentation and Makefile checks**

Run:

```bash
make -n deps-up-apple
make -n deps-up-podman
make -n db-migrate-apple
make -n db-migrate-podman
make -n run-service
make -n run-gateway
git diff --check
```

Expected: every command exits successfully and `git diff --check` produces no output.

Rust source and Cargo manifests are not changed by this plan, so do not rerun the full Rust build,
test, or Clippy gates. The relevant verification is the actual container/application flow and the
documentation/Makefile checks.

**Step 6: Commit any final documentation corrections**

```bash
git add README.md Makefile
git commit -m "docs: polish local setup instructions"
```
