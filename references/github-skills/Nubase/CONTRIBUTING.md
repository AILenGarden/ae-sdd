# Contributing to Nubase

Thanks for your interest in contributing. Nubase is an open-source, AI-native
backend platform — Memory, Database, Storage, and Auth in one self-hostable
service. This guide covers how to get a development environment running and how
to propose changes.

## Ways to Contribute

- Report bugs and request features via GitHub Issues.
- Improve documentation under `docs/`.
- Fix bugs or implement features via pull requests.
- For **security issues**, do not open a public issue — see [SECURITY.md](SECURITY.md).

## Prerequisites

- Java 17
- Maven
- PostgreSQL 15 with the `pgvector` extension
- Node.js and pnpm (for the Studio frontend)
- Docker (optional, for Postgres and the all-in-one image)

## Getting Started

A full walkthrough lives in [`docs/getting-started.md`](docs/getting-started.md).
The short version:

```bash
# 1. Start Postgres (pgvector) — local dev
docker compose -f pg-docker-compose.yml up -d

# 2. Required secrets (development values shown; use real secrets in production)
export PGRST_ENCRYPTION_MASTER_KEY="$(openssl rand -base64 32)"
export METADATA_SERVICE_ROLE_KEY="replace-with-a-long-random-admin-token"
# Optional, only for LLM-powered Memory:
export OPENAI_API_KEY="sk-..."

# 3. Run the backend
mvn spring-boot:run

# 4. Run Studio (in frontend/)
cd frontend && pnpm install && pnpm --filter @nubase/studio dev
```

The backend serves on `:9999`, Studio on `:3000`.

## Building and Testing

```bash
# Backend: compile + run tests
mvn clean verify

# Backend: skip tests
mvn clean package -Dmaven.test.skip=true

# Frontend: build Studio
cd frontend && pnpm --filter @nubase/studio build
```

Please make sure `mvn test` passes and the frontend builds before opening a PR.
Add or update tests for behavior changes.

## Pull Request Process

1. Fork the repo and create a topic branch off `main`
   (e.g. `feat/memory-filters`, `fix/rls-claims`).
2. Keep changes focused — one logical change per PR.
3. Write a clear description: what changed, why, and how you verified it.
4. Ensure the build and tests pass.
5. Link any related issue.

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(mem): add metadata filters to search
fix(auth): reject expired refresh tokens
docs: clarify storage bucket setup
```

Keep commits buildable and scoped to a single concern where practical.

## Code Style

- **Java**: standard conventions; keep methods small and prefer constructor
  injection. Match the style of the surrounding code.
- **TypeScript/React (Studio)**: follow the existing ESLint/Prettier setup in
  `frontend/`.
- Don't commit secrets, build artifacts, or IDE/editor files.

## License

By contributing, you agree that your contributions will be licensed under the
[Apache License 2.0](LICENSE), the same license that covers this project.
