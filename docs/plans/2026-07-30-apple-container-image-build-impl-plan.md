# Apple container Image Build Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add reliable Makefile targets that build all qa-sys images with Apple `container` despite
its recursive build-context archiving defect.

**Architecture:** Package the working tree into a temporary metadata-free tar archive, generate a
temporary transformation of the existing production Dockerfile so it extracts that archive, and
always remove the temporary directory. Keep the standard Dockerfiles as the single source of truth.

**Tech Stack:** GNU Make 3.81, POSIX shell, BSD tar, awk, Apple `container` 1.1,
Dockerfile/BuildKit.

---

### Task 1: Add Apple image build automation

**Files:**
- Modify: `Makefile`

**Step 1: Verify the aggregate target does not exist**

Run: `make -n images-build-apple`

Expected: FAIL with `No rule to make target`.

**Step 2: Add configurable image variables**

Add variables for the development, qa-svc, and Gateway image tags, Apple builder memory, and target
platform. Default the platform to `linux/arm64`.

**Step 3: Add the temporary-context build helper**

The helper must:

- create its directory with `mktemp -d`;
- install a cleanup trap before creating artifacts;
- use `COPYFILE_DISABLE=1` and `tar --no-xattrs`;
- exclude `.git`, `target`, `.env*`, IDE metadata, and AppleDouble files;
- transform only the exact `COPY . .` line;
- write the transformed Dockerfile into the temporary context;
- pass `DEV_IMAGE` as a build arg so tag overrides apply to application builders.
- pass `APPLE_BUILD_PLATFORM` to every `container build` invocation.

**Step 4: Add public targets**

Add `image-build-dev-apple`, `image-build-qa-svc-apple`,
`image-build-gateway-apple`, and `images-build-apple`, then register them in `.PHONY`.
The aggregate target must create one temporary source archive and reuse it for both application
builds.

**Step 5: Verify command expansion**

Run:

```bash
make -n image-build-dev-apple
make -n image-build-qa-svc-apple
make -n image-build-gateway-apple
make -n images-build-apple
```

Expected: PASS; application targets create and clean a temporary directory and never package
excluded paths.

### Task 2: Document and validate the workflow

**Files:**
- Modify: `README.md`
- Modify: `docs/index.md`

**Step 1: Document Apple image build targets**

Explain the aggregate and individual commands, configurable tags, temporary-context behavior, and
the reason direct `container build` is not used for application images.

**Step 2: Build all images**

Run: `make images-build-apple`

Expected: PASS and produce `qa-project-dev:v1.0`, `qa-svc:latest`, and `qa-gateway:latest`.

**Step 3: Verify AMD64 builds**

Run the aggregate target with `APPLE_BUILD_PLATFORM=linux/amd64` and architecture-specific image
tags. Inspect the resulting images and confirm their platform is `linux/amd64`.

**Step 4: Verify runtime contents**

Run one-shot containers that assert both application binaries are executable, configuration
environment variables are correct, and expected diagnostic tools exist.

**Step 5: Verify repository hygiene**

Run:

```bash
git diff --check
git status --short --untracked-files=all
```

Expected: no generated tar archive or temporary build directory appears in the repository.
