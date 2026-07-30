APP_CONFIG_PATH ?= $(CURDIR)/app.yaml
MIGRATION_PATH ?= $(CURDIR)/migrations/20260725064428_db.sql

DEV_PREFIX ?= qa-sys

POSTGRES_IMAGE ?= docker.io/library/postgres:18.4
REDIS_IMAGE ?= docker.io/library/redis:8.8.1-alpine
PULSAR_IMAGE ?= docker.io/apachepulsar/pulsar:4.2.3

POSTGRES_PORT ?= 5432
REDIS_PORT ?= 6379
PULSAR_PORT ?= 6650
PULSAR_HTTP_PORT ?= 8080

DEPS_WAIT_ATTEMPTS ?= 60
DEPS_WAIT_INTERVAL ?= 1

POSTGRES_USER ?= postgres
POSTGRES_PASSWORD ?= postgres
POSTGRES_DATABASE ?= qa_sys
REDIS_PASSWORD ?= redis

POSTGRES_CONTAINER := $(DEV_PREFIX)-postgres
REDIS_CONTAINER := $(DEV_PREFIX)-redis
PULSAR_CONTAINER := $(DEV_PREFIX)-pulsar

POSTGRES_VOLUME := $(DEV_PREFIX)-postgres-data
REDIS_VOLUME := $(DEV_PREFIX)-redis-data
PULSAR_DATA_VOLUME := $(DEV_PREFIX)-pulsar-data
PULSAR_LOG_VOLUME := $(DEV_PREFIX)-pulsar-logs

define remove_containers
@set -e; \
for name in \
	"$(POSTGRES_CONTAINER)" \
	"$(REDIS_CONTAINER)" \
	"$(PULSAR_CONTAINER)"; do \
	if $(1) inspect "$$name" >/dev/null 2>&1; then \
		$(1) rm --force "$$name" >/dev/null; \
	fi; \
done
endef

define ensure_volume
@$(1) volume inspect "$(2)" >/dev/null 2>&1 || \
	$(1) volume create "$(2)" >/dev/null
endef

define run_postgres
@$(1) run -d --name "$(POSTGRES_CONTAINER)" \
	-p "$(POSTGRES_PORT):5432" \
	-e POSTGRES_USER="$(POSTGRES_USER)" \
	-e POSTGRES_PASSWORD="$(POSTGRES_PASSWORD)" \
	-e POSTGRES_DB="$(POSTGRES_DATABASE)" \
	-v "$(POSTGRES_VOLUME):/var/lib/postgresql" \
	"$(POSTGRES_IMAGE)"
endef

define run_redis
@$(1) run -d --name "$(REDIS_CONTAINER)" \
	$(2) \
	-p "$(REDIS_PORT):6379" \
	-v "$(REDIS_VOLUME):/data" \
	"$(REDIS_IMAGE)" \
	$(3) --appendonly yes --requirepass "$(REDIS_PASSWORD)"
endef

define run_pulsar
@$(1) run -d --name "$(PULSAR_CONTAINER)" \
	--user 0:0 \
	$(2) \
	-e "PULSAR_MEM=-Xms512m -Xmx512m -XX:MaxDirectMemorySize=256m" \
	-p "$(PULSAR_PORT):6650" \
	-p "$(PULSAR_HTTP_PORT):8080" \
	-v "$(PULSAR_DATA_VOLUME):/pulsar/data" \
	-v "$(PULSAR_LOG_VOLUME):/pulsar/logs" \
	"$(PULSAR_IMAGE)" \
	bin/pulsar standalone
endef

define wait_for_dependency
@$(1) inspect "$(2)" >/dev/null 2>&1 || { \
	echo "$(4) container $(2) was not found"; \
	exit 1; \
}
@attempt=1; \
until $(1) exec "$(2)" $(3) >/dev/null 2>&1; do \
	if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
		echo "$(4) did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
		exit 1; \
	fi; \
	attempt=$$((attempt + 1)); \
	sleep "$(DEPS_WAIT_INTERVAL)"; \
done
@echo "$(4) is ready"
endef

define migrate_database
@table_count=$$($(1) exec "$(POSTGRES_CONTAINER)" \
	psql -U "$(POSTGRES_USER)" -d "$(POSTGRES_DATABASE)" -Atq -c \
	"SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('questions', 'answers', 'users', 'users_votes');"); \
case "$$table_count" in \
	0) \
		echo "Applying database migration"; \
		$(1) exec -i "$(POSTGRES_CONTAINER)" \
			psql -U "$(POSTGRES_USER)" -d "$(POSTGRES_DATABASE)" \
			--set ON_ERROR_STOP=1 < "$(MIGRATION_PATH)"; \
		;; \
	4) \
		echo "Database migration is already applied"; \
		;; \
	*) \
		echo "Unexpected or partial database migration state: found $$table_count of 4 core tables"; \
		echo "Inspect the database or run make $(2) to recreate local data"; \
		exit 1; \
		;; \
esac
endef

define remove_volumes
@set -e; \
for name in \
	"$(POSTGRES_VOLUME)" \
	"$(REDIS_VOLUME)" \
	"$(PULSAR_DATA_VOLUME)" \
	"$(PULSAR_LOG_VOLUME)"; do \
	if $(1) volume inspect "$$name" >/dev/null 2>&1; then \
		$(1) volume $(2) "$$name" >/dev/null; \
	fi; \
done
endef

check-apple-container:
	@command -v container >/dev/null 2>&1 || { \
		echo "Apple container CLI is not installed"; \
		exit 1; \
	}
	@container system status >/dev/null

deps-up-apple: check-apple-container
	$(call remove_containers,container)
	$(call ensure_volume,container,$(POSTGRES_VOLUME))
	$(call ensure_volume,container,$(REDIS_VOLUME))
	$(call ensure_volume,container,$(PULSAR_DATA_VOLUME))
	$(call ensure_volume,container,$(PULSAR_LOG_VOLUME))
	$(call run_postgres,container)
	$(call run_redis,container,--user 0:0 --entrypoint redis-server,)
	$(call run_pulsar,container,--memory 2g)

deps-status-apple: check-apple-container
	$(call wait_for_dependency,container,$(POSTGRES_CONTAINER),pg_isready -U "$(POSTGRES_USER)" -d "$(POSTGRES_DATABASE)",PostgreSQL)
	$(call wait_for_dependency,container,$(REDIS_CONTAINER),redis-cli -a "$(REDIS_PASSWORD)" --no-auth-warning ping,Redis)
	$(call wait_for_dependency,container,$(PULSAR_CONTAINER),bin/pulsar-admin clusters list,Pulsar)

db-migrate-apple: deps-status-apple
	$(call migrate_database,container,deps-reset-apple)

deps-down-apple: check-apple-container
	$(call remove_containers,container)

deps-reset-apple: deps-down-apple
	@echo "Deleting Apple container volumes for $(DEV_PREFIX) and all data stored in them"
	$(call remove_volumes,container,delete)

check-podman:
	@command -v podman >/dev/null 2>&1 || { \
		echo "Podman is not installed"; \
		exit 1; \
	}
	@podman info >/dev/null

deps-up-podman: check-podman
	$(call remove_containers,podman)
	$(call ensure_volume,podman,$(POSTGRES_VOLUME))
	$(call ensure_volume,podman,$(REDIS_VOLUME))
	$(call ensure_volume,podman,$(PULSAR_DATA_VOLUME))
	$(call ensure_volume,podman,$(PULSAR_LOG_VOLUME))
	$(call run_postgres,podman)
	$(call run_redis,podman,,redis-server)
	$(call run_pulsar,podman,)

deps-status-podman: check-podman
	$(call wait_for_dependency,podman,$(POSTGRES_CONTAINER),pg_isready -U "$(POSTGRES_USER)" -d "$(POSTGRES_DATABASE)",PostgreSQL)
	$(call wait_for_dependency,podman,$(REDIS_CONTAINER),redis-cli -a "$(REDIS_PASSWORD)" --no-auth-warning ping,Redis)
	$(call wait_for_dependency,podman,$(PULSAR_CONTAINER),bin/pulsar-admin clusters list,Pulsar)

db-migrate-podman: deps-status-podman
	$(call migrate_database,podman,deps-reset-podman)

deps-down-podman: check-podman
	$(call remove_containers,podman)

deps-reset-podman: deps-down-podman
	@echo "Deleting Podman volumes for $(DEV_PREFIX) and all data stored in them"
	$(call remove_volumes,podman,rm)

run-service:
	@APP_CONFIG="$(APP_CONFIG_PATH)" cargo run -p qa-svc

run-gateway:
	@QA_CONFIG_DIR="$(CURDIR)/crates/gateway" cargo run -p gateway

build:
	@cargo build

test:
	@cargo nextest run --all-features

test-unit:
	@cargo test -p qa-sys-core
	@cargo test -p qa-svc --lib
	@cargo test -p gateway --lib

test-postgres:
	@cargo test -p qa-svc \
		--test postgres_user_repository \
		--test postgres_question_repository \
		--test postgres_answer_repository \
		--test postgres_vote_repository \
		-- --test-threads=1

test-redis:
	@cargo test -p qa-svc \
		--test redis_sessions \
		--test redis_read_counts \
		-- --test-threads=1

test-pulsar:
	@cargo test -p qa-svc --test pulsar_vote_pipeline -- --test-threads=1

test-grpc:
	@APP_CONFIG="$(APP_CONFIG_PATH)" cargo test -p qa-svc --test grpc_api -- --test-threads=1

test-gateway-integration:
	@APP_CONFIG="$(APP_CONFIG_PATH)" cargo test -p gateway --test http_api -- --test-threads=1

test-e2e:
	@APP_CONFIG="$(APP_CONFIG_PATH)" cargo test -p gateway --test end_to_end_qa_flow -- --test-threads=1

test-integration: test-postgres test-redis test-pulsar test-grpc test-gateway-integration test-e2e

test-all: test-unit test-integration

test-nextest:
	@cargo nextest run --all-features

check-agent-sync:
	@cmp -s CLAUDE.md AGENTS.md || { \
		echo "AGENTS.md must stay in sync with CLAUDE.md"; \
		echo "Update both files with the same shared project instructions."; \
		exit 1; \
	}
	@tmp_dir=$$(mktemp -d); \
	trap 'rm -rf "$$tmp_dir"' EXIT; \
	cp -R .claude/skills "$$tmp_dir/expected-skills"; \
	find "$$tmp_dir/expected-skills" -name SKILL.md -exec perl -0pi -e 's/CLAUDE\.md/AGENTS.md/g; s/Claude/Codex/g; s/claude/codex/g' {} +; \
	diff -ru --exclude agents "$$tmp_dir/expected-skills" .agents/skills || { \
		echo "Codex skills must stay in sync with Claude skills after Claude-to-Codex renaming."; \
		echo "Update .claude/skills first, then mirror the shared content into .agents/skills."; \
		exit 1; \
	}

release:
	@cargo release tag --execute
	@git cliff -o CHANGELOG.md
	@git commit -a -n -m "Update CHANGELOG.md" || true
	@git push origin master
	@cargo release push --execute

update-submodule:
	@git submodule update --init --recursive --remote

.PHONY: check-apple-container deps-up-apple deps-status-apple db-migrate-apple \
	deps-down-apple deps-reset-apple check-podman deps-up-podman \
	deps-status-podman db-migrate-podman deps-down-podman \
	deps-reset-podman run-service run-gateway \
	build test test-unit test-postgres test-redis test-pulsar test-grpc \
	test-gateway-integration test-e2e test-integration test-all test-nextest \
	check-agent-sync release update-submodule
