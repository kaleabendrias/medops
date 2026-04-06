# Hospital Platform Monorepo

Production-oriented local monorepo that runs fully offline with Docker Compose.

- Rocket REST API: `http://localhost:8000`
- Dioxus web app: `http://localhost:8080`
- MySQL: `localhost:3306`

All runtime configuration is checked in. No `.env` file is required.

## Quick Start

```bash
docker compose up -d --build mysql api web
```

Health checks:

```bash
curl -fsS http://localhost:8000/api/v1/health
curl -fsS http://localhost:8080/health
```

Run the full local acceptance pipeline:

```bash
bash run_tests.sh
```

## Verification Prerequisites

Integrated containerized verification requires:

- Docker + Docker Compose plugin
- `bash`, `curl`, `python3`, `sha256sum`
- `google-chrome` for browser-level E2E checks (`API_tests/browser_e2e.sh`)

Non-Docker frontend checks require:

- Rust toolchain (`rustup`, `cargo`)
- wasm target: `rustup target add wasm32-unknown-unknown`
- Optional Dioxus CLI for local serve (`dx`) or Trunk (`trunk`)

## Command Matrix

Integrated containerized verification (authoritative):

```bash
bash unit_tests/run_backend_unit_tests.sh test_reports
bash unit_tests/run_frontend_unit_tests.sh test_reports
bash API_tests/migration_checks.sh test_reports
bash API_tests/authorization_matrix.sh test_reports
bash API_tests/api_integration_tests.sh test_reports
bash API_tests/browser_e2e.sh test_reports
bash API_tests/e2e_smoke.sh test_reports
bash run_tests.sh
```

Expected success signals:

- each command exits `0`
- each suite writes `test_reports/<suite>.json` with `"status":"pass"`
- `bash run_tests.sh` writes `test_reports/summary.json` with `"status":"pass"`
- browser suite writes `test_reports/browser_e2e.json` with happy-path and denied-path pass

Reproducible acceptance evidence is written to `test_reports/`:

- `summary.json` overall status
- `test_pipeline.ndjson` step-by-step pipeline status
- `*.json` and `*.ndjson` suite outputs per test script

## Local Frontend Run (Non-Docker)

The supported integrated path is Docker (`mysql + api + web`) because API data, RBAC seeds, and migration checks are containerized and deterministic.

For local frontend-only development against a running API, you can run Rust checks/tests directly:

```bash
cargo test -p web-app
```

Optional local frontend commands:

```bash
cargo test -p web-app --bin web-app
cd services/web && trunk serve --release
cd services/web && dx serve --platform web
```

Expected success signals:

- tests: `test result: ok.`
- local serve: startup banner with local URL and no compile errors

If your machine has the Dioxus CLI installed, you can also run a local dev server from `services/web`:

```bash
dx serve --platform web
```

If `dx` or `trunk` is not installed, use Docker as the runtime validation boundary.

## Helper Scripts

```bash
./scripts/stack.sh build     # Build all images
./scripts/stack.sh up        # Start full stack
./scripts/stack.sh down      # Stop stack
./scripts/stack.sh logs      # Tail all logs
./scripts/stack.sh status    # Show service status
./scripts/stack.sh reset     # Stop + remove volumes
./scripts/stack.sh mysql     # Open mysql shell in container
```

## Services and Responsibilities

- `mysql`: persistence for all domain tables, migrations, and deterministic seed data.
- `api`: authentication, RBAC, object-level authorization, encryption/masking, lifecycle policy, append-only auditing.
- `web`: intranet UI that consumes API contracts only.
- `test_runner`: isolated Rust toolchain used by automated scripts.

## Seeded Users

> **NON-PRODUCTION CREDENTIALS.** The credentials below are for local development
> and offline testing only. They must **never** be reused in staging, production, or
> any internet-facing environment. For deployment, override all passwords and secrets
> via environment variables (see *Credential Overrides* below).

Users are seeded from `services/api/migrations`:

- `admin` / `Admin#OfflinePass123`
- `member1` / `Admin#OfflinePass123`
- `employee1` / `Admin#OfflinePass123`
- `clinical1` / `Admin#OfflinePass123`
- `cafeteria1` / `Admin#OfflinePass123`
- `lockout_user` / `Admin#OfflinePass123`

### Credential Overrides

All database and application credentials can be overridden via environment variables
at deploy time without modifying checked-in files:

```bash
# Example: override MySQL credentials for a deployment profile
export MYSQL_ROOT_PASSWORD=<strong-random-secret>
export MYSQL_PASSWORD=<strong-random-secret>
export DATABASE_URL=mysql://app_user:<password>@mysql:3306/hospital_platform

docker compose up -d
```

A `docker-compose.override.yml` can also be used to inject secrets per environment.

## Role Verification (E2E)

- Admin can run privileged endpoints (`/patients/search`, `/ingestion/tasks`, reveal-sensitive patient reads).
- Clinical users can access patient workflows but are denied dining pricing/inventory management endpoints.
- Cafeteria users can manage dining/orders/campaigns but are denied clinical records.
- Catalog metadata routes are auth + authorization protected.
- Lockout behavior is enforced after repeated failed login attempts.

These are continuously verified by `API_tests/authorization_matrix.sh` and `API_tests/api_integration_tests.sh`.

## Architecture Overview

API layers in `services/api/src` are intentionally strict:

- `contracts`: request/response DTOs and HTTP error mapping.
- `services`: use-case orchestration, policy checks, lifecycle rules.
- `repositories`: application persistence traits.
- `infrastructure`: MySQL repository adapter and local security primitives.
- `routes`: Rocket handlers and auth extraction.

### Governance Tiered Storage

The governance data model uses a **single physical table** (`governance_records`) with a
`tier` column (`raw`, `cleaned`, `analytics`) and a self-referential `lineage_source_id`
foreign key for lineage tracking. This is an accepted architectural deviation from
separate physical tables, chosen for:

1. **Lineage integrity** — cross-tier FK references stay within one table.
2. **Uniform policy** — append-only audit and tombstone logic apply identically.
3. **Operational simplicity** — appropriate for a single-hospital offline deployment.

Logical separation is provided via tiered views (`governance_raw`, `governance_cleaned`,
`governance_analytics`) defined in migration `014_governance_tiered_views.sql`, each
exposing its tier with explicit lineage joins to source records.

## Intranet API Surface

Implemented domains include:

- authentication and session validation
- RBAC and permission-gated operations
- patient CRUD, assignment-based object isolation, masked-by-default sensitive fields with privileged reveal
- bed board transitions with legal state-machine enforcement
- dining orders with idempotent create and versioned status updates
- MySQL-authoritative patient attachment payload storage with legacy filesystem fallback for historical rows
- ingestion task manager (create, update, versions, rollback, run)
- governance lineage/tombstone behavior
- experimentation telemetry and analytics endpoints
- append-only audit log behavior and retention constraints

### Retention API

| Method | Path | Permission | Description |
|--------|------|------------|-------------|
| GET | `/api/v1/retention` | authenticated | Returns current retention settings snapshot (audit, session, patient record days) |
| GET | `/api/v1/retention/policies` | `audit.read` | Lists all retention policies from database |
| PUT | `/api/v1/retention/policies/<key>/<years>` | `retention.manage` | Upserts a retention policy by key and minimum years |
| GET | `/api/v1/analytics/retention` | `audit.read` | Returns user retention cohort metrics (1-day and 7-day active users) |

All retention endpoints require authentication. Policy listing requires `audit.read`. Policy mutation (`PUT`) requires `retention.manage` permission and enforces a minimum of `clinical_years_min` (default 7) for clinical record policies.

## Configuration Notes

- Credentials and configuration are committed for local/offline execution.
- Field encryption uses a local keyring strategy in the API service.
- Session/auth/retention defaults live in `services/api/config/default.toml`.

## Troubleshooting

- API container unhealthy after migration change:
  - `docker compose logs --no-color api`
  - check SQL syntax in newest file under `services/api/migrations/`.
- Failing integration script:
  - inspect `test_reports/*.json` and `test_reports/*.ndjson`.
- Attachment upload failures:
  - confirm file extension and MIME match (PDF/JPG/PNG) and size <= 25 MB.
  - verify `patient_attachments.payload_blob` exists (`bash API_tests/migration_checks.sh test_reports`).
  - legacy rows with `payload_blob IS NULL` must still have a readable `storage_path`.
- Frontend-only checks:
  - `cargo test -p web-app`
  - optional local dev server: `dx serve --platform web` (if Dioxus CLI available).
