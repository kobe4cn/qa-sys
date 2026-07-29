# Crates Test Coverage Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add meaningful unit, component-integration, gRPC, HTTP, and end-to-end tests for the crates workspace without contaminating existing service data.

**Architecture:** Tests are layered so failures identify the responsible boundary: pure logic is tested in colocated unit modules, repositories are tested against isolated PostgreSQL and Redis resources, Pulsar uses a dedicated topic, and transport contracts are tested through real Axum and Tonic services. Any test that demonstrates a production defect is left failing until the proposed production change receives explicit user approval.

**Tech Stack:** Rust 2024, Cargo test/nextest, SQLx, PostgreSQL, Redis, Pulsar, Tonic, Axum, Validator.

---

### Task 1: Core unit tests

**Files:**

- Modify: `crates/core/src/aes.rs`
- Modify: `crates/core/src/config.rs`
- Modify: `crates/core/src/xpgsql.rs`
- Modify: `crates/core/src/xredis.rs`
- Modify: `crates/core/src/xpulsar.rs`

**Steps:**

1. Extend AES tests with all key sizes, empty/Unicode/large plaintext, malformed Base64, and corrupted ciphertext.
2. Add configuration file success and failure tests using unique temporary files.
3. Add builder boundary tests for empty and valid PostgreSQL, Redis, and Pulsar DSNs.
4. Run `cargo test -p qa-sys-core`.
5. If a test requires production behavior changes, stop and request approval with the failing output and proposed patch.

### Task 2: Domain and pure helper unit tests

**Files:**

- Modify: `crates/qa-svc/src/domain/entity/answer.rs`
- Modify: `crates/qa-svc/src/domain/entity/vote_message.rs`
- Modify: `crates/qa-svc/src/core/user.rs`
- Modify: `crates/qa-svc/src/core/user_vote.rs`

**Steps:**

1. Test pagination metadata for empty, exact, partial, last-page, and invalid page-size cases.
2. Test vote message serialization and malformed payload rejection.
3. Test SQL placeholder generation for zero, one, and multiple values.
4. Run the smallest matching `cargo test -p qa-svc <test-name>` command after each group.
5. Stop before modifying production behavior for any demonstrated defect.

### Task 3: Gateway unit tests

**Files:**

- Modify: `crates/gateway/src/handler/qa.rs`
- Modify: `crates/gateway/src/handler/json_or_form.rs`
- Modify: `crates/gateway/src/middleware/header.rs`
- Modify: `crates/gateway/src/router/router.rs`

**Steps:**

1. Test DTO validation boundaries and GatewayError status mapping.
2. Test JSON, form, content-type, malformed body, and validation extractor behavior.
3. Test cache headers on success and error responses.
4. Extend router authentication and route classification tests.
5. Run `cargo test -p gateway`.
6. Stop before correcting any production behavior exposed by tests.

### Task 4: PostgreSQL repository integration tests

**Files:**

- Create: `crates/qa-svc/tests/common/mod.rs`
- Create: `crates/qa-svc/tests/postgres_user_repository.rs`
- Create: `crates/qa-svc/tests/postgres_question_repository.rs`
- Create: `crates/qa-svc/tests/postgres_answer_repository.rs`
- Create: `crates/qa-svc/tests/postgres_vote_repository.rs`

**Steps:**

1. Read PostgreSQL connection information from `app.yaml`.
2. Create isolated per-test databases and apply `migrations/`.
3. Cover CRUD, ownership, pagination, empty collections, constraints, and transaction behavior.
4. Run each integration test file independently.
5. Present SQL or domain failures and request production-change approval before fixing them.

### Task 5: Redis integration tests

**Files:**

- Create: `crates/qa-svc/tests/redis_sessions.rs`
- Create: `crates/qa-svc/tests/redis_read_counts.rs`

**Steps:**

1. Derive the Redis server credentials from `app.yaml` and select DB 15.
2. Clear only DB 15 at test setup and teardown.
3. Cover session lifecycle, TTL, malformed session data, read increments, and PostgreSQL flush behavior.
4. Run each Redis integration test independently.
5. Stop before any production fix exposed by the tests.

### Task 6: Application and gRPC tests

**Files:**

- Modify: `crates/qa-svc/src/application/app.rs`
- Create: `crates/qa-svc/tests/grpc_api.rs`

**Steps:**

1. Add repository fakes in a colocated unit-test module.
2. Cover token parsing/expiration, registration, login, logout, verification, questions, answers, and votes.
3. Start a Tonic server on an ephemeral port for protocol-level integration tests.
4. Run `cargo test -p qa-svc`.
5. Stop and request approval for any required clock, constructor, error-mapping, or business-logic change.

### Task 7: Pulsar vote pipeline tests

**Files:**

- Create: `crates/qa-svc/tests/pulsar_vote_pipeline.rs`

**Steps:**

1. Use a dedicated test topic and subscription.
2. Test serialization, publish, consume, database mutation, acknowledgement, malformed messages, and shutdown.
3. Request approval before making the hard-coded topic/subscription configurable.
4. Run the Pulsar test with a bounded timeout.

### Task 8: Gateway HTTP and end-to-end tests

**Files:**

- Create: `crates/gateway/tests/common/mod.rs`
- Create: `crates/gateway/tests/http_api.rs`
- Create: `crates/gateway/tests/end_to_end_qa_flow.rs`

**Steps:**

1. Start a controllable fake gRPC server and test all HTTP/protobuf mappings.
2. Test route construction, methods, authentication, no-cache headers, fallbacks, and authenticated identity propagation.
3. Start the real gRPC application against isolated PostgreSQL/Redis/Pulsar resources.
4. Cover register/login/verify/logout, question CRUD, answer CRUD, vote/unvote, authorization, and error paths.
5. Stop before changing any production path, identity, DTO, or handler behavior exposed by the tests.

### Task 9: Automation and final verification

**Files:**

- Modify: `Makefile`
- Modify as required: crate `[dev-dependencies]`

**Steps:**

1. Verify current official documentation and versions before adding test dependencies.
2. Add Makefile targets for unit, PostgreSQL, Redis, Pulsar, integration, end-to-end, and complete test runs.
3. Run `cargo build`.
4. Run `cargo test`.
5. Run `cargo +nightly fmt -- --check`.
6. Run `cargo clippy -- -D warnings` and the stricter pedantic lint where it adds signal.
7. If dependencies or lockfiles changed, run `cargo audit` and `cargo deny check`.
8. Inspect all changed files and report any pre-existing gate failures separately.
