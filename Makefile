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

POSTGRES_CONTAINER := $(DEV_PREFIX)-postgres
REDIS_CONTAINER := $(DEV_PREFIX)-redis
PULSAR_CONTAINER := $(DEV_PREFIX)-pulsar

POSTGRES_VOLUME := $(DEV_PREFIX)-postgres-data
REDIS_VOLUME := $(DEV_PREFIX)-redis-data
PULSAR_DATA_VOLUME := $(DEV_PREFIX)-pulsar-data
PULSAR_LOG_VOLUME := $(DEV_PREFIX)-pulsar-logs

check-apple-container:
	@command -v container >/dev/null 2>&1 || { \
		echo "Apple container CLI is not installed"; \
		exit 1; \
	}
	@container system status >/dev/null

deps-up-apple: check-apple-container
	@set -e; \
	for name in \
		"$(POSTGRES_CONTAINER)" \
		"$(REDIS_CONTAINER)" \
		"$(PULSAR_CONTAINER)"; do \
		if container inspect "$$name" >/dev/null 2>&1; then \
			container delete --force "$$name" >/dev/null; \
		fi; \
	done
	@container volume inspect "$(POSTGRES_VOLUME)" >/dev/null 2>&1 || \
		container volume create "$(POSTGRES_VOLUME)" >/dev/null
	@container volume inspect "$(REDIS_VOLUME)" >/dev/null 2>&1 || \
		container volume create "$(REDIS_VOLUME)" >/dev/null
	@container volume inspect "$(PULSAR_DATA_VOLUME)" >/dev/null 2>&1 || \
		container volume create "$(PULSAR_DATA_VOLUME)" >/dev/null
	@container volume inspect "$(PULSAR_LOG_VOLUME)" >/dev/null 2>&1 || \
		container volume create "$(PULSAR_LOG_VOLUME)" >/dev/null
	@container run -d --name "$(POSTGRES_CONTAINER)" \
		-p "$(POSTGRES_PORT):5432" \
		-e POSTGRES_USER=postgres \
		-e POSTGRES_PASSWORD=postgres \
		-e POSTGRES_DB=qa_sys \
		-v "$(POSTGRES_VOLUME):/var/lib/postgresql" \
		"$(POSTGRES_IMAGE)"
	@container run -d --name "$(REDIS_CONTAINER)" \
		--user 0:0 \
		--entrypoint redis-server \
		-p "$(REDIS_PORT):6379" \
		-v "$(REDIS_VOLUME):/data" \
		"$(REDIS_IMAGE)" \
		--appendonly yes --requirepass redis
	@container run -d --name "$(PULSAR_CONTAINER)" \
		--user 0:0 \
		--memory 2g \
		-e "PULSAR_MEM=-Xms512m -Xmx512m -XX:MaxDirectMemorySize=256m" \
		-p "$(PULSAR_PORT):6650" \
		-p "$(PULSAR_HTTP_PORT):8080" \
		-v "$(PULSAR_DATA_VOLUME):/pulsar/data" \
		-v "$(PULSAR_LOG_VOLUME):/pulsar/logs" \
		"$(PULSAR_IMAGE)" \
		bin/pulsar standalone

deps-status-apple: check-apple-container
	@container inspect "$(POSTGRES_CONTAINER)" >/dev/null 2>&1 || { \
		echo "PostgreSQL container $(POSTGRES_CONTAINER) was not found"; \
		exit 1; \
	}
	@attempt=1; \
	until container exec "$(POSTGRES_CONTAINER)" \
		pg_isready -U postgres -d qa_sys >/dev/null 2>&1; do \
		if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
			echo "PostgreSQL did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
			exit 1; \
		fi; \
		attempt=$$((attempt + 1)); \
		sleep "$(DEPS_WAIT_INTERVAL)"; \
	done
	@echo "PostgreSQL is ready"
	@container inspect "$(REDIS_CONTAINER)" >/dev/null 2>&1 || { \
		echo "Redis container $(REDIS_CONTAINER) was not found"; \
		exit 1; \
	}
	@attempt=1; \
	until container exec "$(REDIS_CONTAINER)" \
		redis-cli -a redis --no-auth-warning ping >/dev/null 2>&1; do \
		if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
			echo "Redis did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
			exit 1; \
		fi; \
		attempt=$$((attempt + 1)); \
		sleep "$(DEPS_WAIT_INTERVAL)"; \
	done
	@echo "Redis is ready"
	@container inspect "$(PULSAR_CONTAINER)" >/dev/null 2>&1 || { \
		echo "Pulsar container $(PULSAR_CONTAINER) was not found"; \
		exit 1; \
	}
	@attempt=1; \
	until container exec "$(PULSAR_CONTAINER)" \
		bin/pulsar-admin clusters list >/dev/null 2>&1; do \
		if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
			echo "Pulsar did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
			exit 1; \
		fi; \
		attempt=$$((attempt + 1)); \
		sleep "$(DEPS_WAIT_INTERVAL)"; \
	done
	@echo "Pulsar is ready"

db-migrate-apple: deps-status-apple
	@table_count=$$(container exec "$(POSTGRES_CONTAINER)" \
		psql -U postgres -d qa_sys -Atq -c \
		"SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('questions', 'answers', 'users', 'users_votes');"); \
	case "$$table_count" in \
		0) \
			echo "Applying database migration"; \
			container exec -i "$(POSTGRES_CONTAINER)" \
				psql -U postgres -d qa_sys --set ON_ERROR_STOP=1 \
				< "$(MIGRATION_PATH)"; \
			;; \
		4) \
			echo "Database migration is already applied"; \
			;; \
		*) \
			echo "Unexpected or partial database migration state: found $$table_count of 4 core tables"; \
			echo "Inspect the database or run make deps-reset-apple to recreate local data"; \
			exit 1; \
			;; \
	esac

deps-down-apple: check-apple-container
	@set -e; \
	for name in \
		"$(POSTGRES_CONTAINER)" \
		"$(REDIS_CONTAINER)" \
		"$(PULSAR_CONTAINER)"; do \
		if container inspect "$$name" >/dev/null 2>&1; then \
			container delete --force "$$name" >/dev/null; \
		fi; \
	done

deps-reset-apple: deps-down-apple
	@echo "Deleting Apple container volumes for $(DEV_PREFIX) and all data stored in them"
	@set -e; \
	for name in \
		"$(POSTGRES_VOLUME)" \
		"$(REDIS_VOLUME)" \
		"$(PULSAR_DATA_VOLUME)" \
		"$(PULSAR_LOG_VOLUME)"; do \
		if container volume inspect "$$name" >/dev/null 2>&1; then \
			container volume delete "$$name" >/dev/null; \
		fi; \
	done

check-podman:
	@command -v podman >/dev/null 2>&1 || { \
		echo "Podman is not installed"; \
		exit 1; \
	}
	@podman info >/dev/null

deps-up-podman: check-podman
	@set -e; \
	for name in \
		"$(POSTGRES_CONTAINER)" \
		"$(REDIS_CONTAINER)" \
		"$(PULSAR_CONTAINER)"; do \
		if podman container exists "$$name"; then \
			podman rm --force "$$name" >/dev/null; \
		fi; \
	done
	@podman volume exists "$(POSTGRES_VOLUME)" || \
		podman volume create "$(POSTGRES_VOLUME)" >/dev/null
	@podman volume exists "$(REDIS_VOLUME)" || \
		podman volume create "$(REDIS_VOLUME)" >/dev/null
	@podman volume exists "$(PULSAR_DATA_VOLUME)" || \
		podman volume create "$(PULSAR_DATA_VOLUME)" >/dev/null
	@podman volume exists "$(PULSAR_LOG_VOLUME)" || \
		podman volume create "$(PULSAR_LOG_VOLUME)" >/dev/null
	@podman run -d --name "$(POSTGRES_CONTAINER)" \
		-p "$(POSTGRES_PORT):5432" \
		-e POSTGRES_USER=postgres \
		-e POSTGRES_PASSWORD=postgres \
		-e POSTGRES_DB=qa_sys \
		-v "$(POSTGRES_VOLUME):/var/lib/postgresql" \
		"$(POSTGRES_IMAGE)"
	@podman run -d --name "$(REDIS_CONTAINER)" \
		-p "$(REDIS_PORT):6379" \
		-v "$(REDIS_VOLUME):/data" \
		"$(REDIS_IMAGE)" \
		redis-server --appendonly yes --requirepass redis
	@podman run -d --name "$(PULSAR_CONTAINER)" \
		--user 0:0 \
		-e "PULSAR_MEM=-Xms512m -Xmx512m -XX:MaxDirectMemorySize=256m" \
		-p "$(PULSAR_PORT):6650" \
		-p "$(PULSAR_HTTP_PORT):8080" \
		-v "$(PULSAR_DATA_VOLUME):/pulsar/data" \
		-v "$(PULSAR_LOG_VOLUME):/pulsar/logs" \
		"$(PULSAR_IMAGE)" \
		bin/pulsar standalone

deps-status-podman: check-podman
	@podman container exists "$(POSTGRES_CONTAINER)" || { \
		echo "PostgreSQL container $(POSTGRES_CONTAINER) was not found"; \
		exit 1; \
	}
	@attempt=1; \
	until podman exec "$(POSTGRES_CONTAINER)" \
		pg_isready -U postgres -d qa_sys >/dev/null 2>&1; do \
		if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
			echo "PostgreSQL did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
			exit 1; \
		fi; \
		attempt=$$((attempt + 1)); \
		sleep "$(DEPS_WAIT_INTERVAL)"; \
	done
	@echo "PostgreSQL is ready"
	@podman container exists "$(REDIS_CONTAINER)" || { \
		echo "Redis container $(REDIS_CONTAINER) was not found"; \
		exit 1; \
	}
	@attempt=1; \
	until podman exec "$(REDIS_CONTAINER)" \
		redis-cli -a redis --no-auth-warning ping >/dev/null 2>&1; do \
		if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
			echo "Redis did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
			exit 1; \
		fi; \
		attempt=$$((attempt + 1)); \
		sleep "$(DEPS_WAIT_INTERVAL)"; \
	done
	@echo "Redis is ready"
	@podman container exists "$(PULSAR_CONTAINER)" || { \
		echo "Pulsar container $(PULSAR_CONTAINER) was not found"; \
		exit 1; \
	}
	@attempt=1; \
	until podman exec "$(PULSAR_CONTAINER)" \
		bin/pulsar-admin clusters list >/dev/null 2>&1; do \
		if [ "$$attempt" -ge "$(DEPS_WAIT_ATTEMPTS)" ]; then \
			echo "Pulsar did not become ready after $(DEPS_WAIT_ATTEMPTS) attempts"; \
			exit 1; \
		fi; \
		attempt=$$((attempt + 1)); \
		sleep "$(DEPS_WAIT_INTERVAL)"; \
	done
	@echo "Pulsar is ready"

db-migrate-podman: deps-status-podman
	@table_count=$$(podman exec "$(POSTGRES_CONTAINER)" \
		psql -U postgres -d qa_sys -Atq -c \
		"SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('questions', 'answers', 'users', 'users_votes');"); \
	case "$$table_count" in \
		0) \
			echo "Applying database migration"; \
			podman exec -i "$(POSTGRES_CONTAINER)" \
				psql -U postgres -d qa_sys --set ON_ERROR_STOP=1 \
				< "$(MIGRATION_PATH)"; \
			;; \
		4) \
			echo "Database migration is already applied"; \
			;; \
		*) \
			echo "Unexpected or partial database migration state: found $$table_count of 4 core tables"; \
			echo "Inspect the database or run make deps-reset-podman to recreate local data"; \
			exit 1; \
			;; \
	esac

deps-down-podman: check-podman
	@set -e; \
	for name in \
		"$(POSTGRES_CONTAINER)" \
		"$(REDIS_CONTAINER)" \
		"$(PULSAR_CONTAINER)"; do \
		if podman container exists "$$name"; then \
			podman rm --force "$$name" >/dev/null; \
		fi; \
	done

deps-reset-podman: deps-down-podman
	@echo "Deleting Podman volumes for $(DEV_PREFIX) and all data stored in them"
	@set -e; \
	for name in \
		"$(POSTGRES_VOLUME)" \
		"$(REDIS_VOLUME)" \
		"$(PULSAR_DATA_VOLUME)" \
		"$(PULSAR_LOG_VOLUME)"; do \
		if podman volume exists "$$name"; then \
			podman volume rm "$$name" >/dev/null; \
		fi; \
	done

run-service:
	@APP_CONFIG="$(APP_CONFIG_PATH)" cargo run -p qa-svc

run-gateway:
	@QA_CONFIG_DIR="$(CURDIR)/crates/gateway" cargo run -p gateway

build:
	@cargo build

test: test-all

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
