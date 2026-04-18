fullstack

# Hospital Platform

A full-stack, offline-capable healthcare platform with patient management, dining/cafeteria ordering, bed-board tracking, A/B experimentation, governance/audit trails, and analytics. Runs entirely inside Docker Compose with no external dependencies.

## Architecture & Tech Stack

* **Frontend:** Dioxus 0.6 (Rust → WASM), served by Nginx 1.27 on port 8443 (HTTPS/TLS). Port 8080 is retained internally for health-checks and test runners. Nginx reverse-proxies `/api/` to the backend for same-origin requests.
* **Backend:** Rocket 0.5 (Rust) REST API on port 8000. Argon2id authentication, HttpOnly+Secure cookie session management, CSRF token validation, AES-GCM field-level encryption, role-based access control, append-only audit logging.
* **Database:** MySQL 8.4 on port 3306. 23 schema migrations, append-only triggers on audit tables, autonomous campaign-closure event.
* **Containerization:** Docker & Docker Compose (required — the stack runs fully offline)

## Request Flow

```
Browser (HTTPS :8443)
  └─▶ Nginx 1.27  (TLS termination, static WASM bundle, /api/* reverse-proxy)
        └─▶ Rocket API  (http://api:8000, internal Docker network only)
                └─▶ MySQL 8.4  (mysql:3306, internal Docker network only)
```

1. The browser loads the Dioxus WASM bundle (`index.html`, `*.wasm`, `*.js`) directly from Nginx.
2. All API calls from the WASM app are relative paths (`/api/v1/...`). Nginx matches the `/api/` prefix and proxies them to `http://api:8000`, preserving the session cookie and CSRF header.
3. Rocket validates the session cookie, checks RBAC permissions, executes business logic, and queries MySQL over the internal Docker network.
4. MySQL is never reachable from the host — only the API container and test containers connect to it.

## Project Structure

```text
.
├── crates/
│   └── contracts/          # Shared DTOs used by both API and web crates
├── services/
│   ├── api/                # Rocket REST API (Dockerfile, migrations, src/)
│   └── web/                # Dioxus WASM frontend (Dockerfile, nginx.conf, src/)
├── API_tests/              # Integration, authorization matrix, E2E, and browser E2E tests
├── unit_tests/             # Backend and frontend unit test runners
├── mysql-init/             # MySQL initialization scripts (event scheduler grant)
├── scripts/                # Helper scripts (stack.sh)
├── test_reports/           # Test output (JSON / NDJSON) written by run_tests.sh
├── docker-compose.yml      # Multi-container orchestration
├── run_tests.sh            # Standardized test execution script
└── README.md               # Project documentation
```

## Prerequisites

This project is **zero-config** — no host-side toolchains, language runtimes, or manual database initialization are required. Everything runs inside Docker Compose containers.

* [Docker](https://docs.docker.com/get-docker/) (with BuildKit)
* [Docker Compose](https://docs.docker.com/compose/install/) v2+

No `curl`, `mysql`, `node`, `cargo`, or `python3` are needed on the host.

## Running the Application

1. **Build and start all containers:**

   ```bash
   docker-compose up
   ```

2. **Access the app:**

   **External (host browser / developer access):**
   * Frontend: `https://localhost:8443` — the only host-exposed port; accept the self-signed certificate warning in your browser.
   * API via proxy: `https://localhost:8443/api/v1` — requests are forwarded to the backend by nginx.

   **Internal (container-to-container networking only):**
   * Backend API: `http://api:8000` — reachable only within the Docker network; not exposed on the host. Test containers and nginx use this address.
   * Database: `mysql:3306` — reachable only within the Docker network.

3. **Stop the application:**

   ```bash
   docker-compose down -v
   ```

## End-User Workflow Verification

Once the stack is running, verify the full user journey manually:

1. **Sign in** — open `https://localhost:8443` in a browser (accept the self-signed certificate warning); sign in as `admin` / `Admin#OfflinePass123`. You should be redirected to the Dashboard.
2. **Role-gated navigation** — confirm the sidebar shows modules appropriate for the admin role (Orders, Patients, Cafeteria, Admin, etc.).
3. **Patient search** — navigate to Patients and search for a name (e.g. `john`); results should appear without errors.
4. **Cafeteria order** — navigate to Orders and place a test order; a confirmation or success banner should appear.
5. **Sign out** — click Sign Out; you should be returned to the login screen and further navigation should be blocked.
6. **Non-admin restriction** — sign in as `member1`; confirm that admin-only sections (e.g. Admin panel) are absent from the sidebar.
7. **Sensitive data reveal** — sign in as `clinical1`; verify that patient detail views show sensitive clinical fields (controlled by the `reveal_sensitive` entitlement).

## Testing

All unit, integration, and E2E tests are executed via a single standardized shell script. The script resets containers, rebuilds images, waits for health checks, then runs each test suite in order.

Make sure the script is executable, then run it:

```bash
chmod +x run_tests.sh
./run_tests.sh
```

The script exits `0` on full success and non-zero on the first failure. A `test_reports/summary.json` file is written with `{"status":"pass"}` or `{"status":"fail","failed_step":"<step>"}`.

### Test suites executed by `run_tests.sh`

| Step | Script | What it covers |
| :--- | :--- | :--- |
| `backend_unit_tests` | `unit_tests/run_backend_unit_tests.sh` | Rust `#[test]` functions in the API crate |
| `frontend_unit_tests` | `unit_tests/run_frontend_unit_tests.sh` | Rust `#[test]` functions in the web crate (including `*.test.rs` files) |
| `migration_checks` | `API_tests/migration_checks.sh` | All 23 SQL migrations, schema expectations, append-only triggers |
| `authorization_matrix_checks` | `API_tests/authorization_matrix.sh` | Role-based access control for every role |
| `api_integration_tests` | `API_tests/api_integration_tests.sh` | Full API surface: auth, patients, bedboard, cafeteria, orders, campaigns, experiments (variant/assign/backtrack), governance, ingestion, analytics, retention, audit |
| `e2e_smoke` | `API_tests/e2e_smoke.sh` | Role-journey smoke tests (admin, clinical, cafeteria, member) |
| `browser_e2e` _(Integration/Proxy Verification)_ | `API_tests/browser_e2e.sh` | Transport-level curl checks: nginx proxy reachability, same-origin login, SPA routing, cookie forwarding, RBAC enforcement through the proxy. Uses `curl -sk` — no real browser. |
| `playwright_e2e` | `API_tests/playwright_e2e.sh` | True browser-level DOM interactions via Playwright + Chromium across four role journeys: Clinical (patient search + DOM state), Admin (all-nav visibility + patient field masking), Cafeteria (dining access + patient section hidden), Member (restricted sidebar + sign-out). |

### Running individual suites

All test suites run inside Docker — no host-level tools (`curl`, `python3`, `bash`) are required.

```bash
# API integration tests only (stack must already be running)
docker compose run --rm -T integration_tests bash API_tests/api_integration_tests.sh test_reports

# Integration/Proxy Verification (curl-based — checks nginx proxy, session cookie, RBAC)
docker compose run --rm -T integration_tests bash API_tests/browser_e2e.sh test_reports

# Playwright browser E2E (real Chromium DOM interactions — stack must be running)
docker compose run --rm -T playwright_tests bash API_tests/playwright_e2e.sh test_reports

# Frontend unit tests (runs inside the web build container)
docker compose run --rm web cargo test --manifest-path services/web/Cargo.toml
```

## Seeded Credentials

The database is pre-seeded with the following test users on startup.

| Role | Username | Password | Notes |
| :--- | :--- | :--- | :--- |
| **Admin** | `admin` | `Admin#OfflinePass123` | Full access to all modules including admin, experiments, analytics, ingestion |
| **Member** | `member1` | `Admin#OfflinePass123` | Self-service orders; no access to clinical or admin routes |
| **Employee** | `employee1` | `Admin#OfflinePass123` | Standard employee entitlements |
| **Clinical** | `clinical1` | `Admin#OfflinePass123` | Patient management, visit notes, clinical data reveal |
| **Cafeteria** | `cafeteria1` | `Admin#OfflinePass123` | Cafeteria management, orders; no patient search or clinical data |
| **Locked** | `lockout_user` | `Admin#OfflinePass123` | Used to test 5-attempt lockout and 15-minute lockout enforcement |

## Manual API Verification

Use these `curl` examples to verify core backend functions from the host. All commands go through Nginx (`https://localhost:8443/api/v1`) and require `-k` to accept the self-signed certificate. To call the backend directly from inside a running container, replace `https://localhost:8443/api` with `http://api:8000/api` and drop `-k`.

### Authentication — `POST /api/v1/auth/login`

```bash
curl -ks -c /tmp/cookies.txt \
  -X POST https://localhost:8443/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin#OfflinePass123"}'
```

Expected response shape:

```json
{
  "csrf_token": "a3f8...64-hex-chars...b921",
  "user_id": 1,
  "username": "admin",
  "role": "admin",
  "expires_in_minutes": 480
}
```

The `csrf_token` (64 hex characters) must be sent as the `X-CSRF-Token` header on every state-changing request (POST / PUT / DELETE / PATCH). The session is maintained via an `HttpOnly` cookie (`hospital_session`) returned in the `Set-Cookie` response header.

### Patient Search — `GET /api/v1/patients/search`

```bash
# Capture the session cookie and CSRF token from login
COOKIE=$(curl -ks -D - -X POST https://localhost:8443/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin#OfflinePass123"}' \
  | grep -i 'set-cookie:' | grep hospital_session \
  | sed 's/.*hospital_session=\([^;]*\).*/\1/' | tr -d '\r')

curl -ks "https://localhost:8443/api/v1/patients/search?q=john" \
  --cookie "hospital_session=$COOKIE"
```

Expected response shape (array; may be empty on a fresh database):

```json
[
  {
    "id": 1,
    "mrn": "***5678",
    "display_name": "John Doe"
  }
]
```

Sensitive fields (`allergies`, `contraindications`, `history`) are returned on the full patient profile endpoint (`GET /patients/{id}`) and are masked as `"[REDACTED - privileged reveal required]"` unless the caller holds the `reveal_sensitive` entitlement. Use `?reveal_sensitive=true` with a clinical1 session to see clear values.

### Audit Log Retrieval — `GET /api/v1/audits`

```bash
curl -ks "https://localhost:8443/api/v1/audits" \
  --cookie "hospital_session=$COOKIE"
```

Expected response shape (append-only; entries accumulate as the system is used):

```json
[
  {
    "id": 42,
    "action_type": "patient.edit",
    "entity_type": "patient",
    "entity_id": "1",
    "actor_username": "admin",
    "created_at": "2025-01-15 14:30:00"
  }
]
```

Every write operation (patient edits, governance creates, user disables, attachment uploads, etc.) emits an audit event. The `entry_hash` chain guarantees that no record can be deleted or modified without detection.

## Troubleshooting

### Self-Signed TLS Certificate

The Nginx container uses a self-signed certificate. Browsers and `curl` reject it by default.

* **Browser** — click **Advanced** → **Proceed to localhost (unsafe)** (Chrome / Edge) or **Accept the Risk and Continue** (Firefox).
* **curl** — add `-k` (or `--insecure`) to skip certificate verification:
  ```bash
  curl -ks https://localhost:8443/api/v1/health
  ```
* **Playwright / automated tests** — the Playwright config sets `ignoreHTTPSErrors: true`; no manual action is required.

### Containers Not Healthy

If services do not reach the `healthy` state within ~2 minutes:

```bash
# Check each container's health
docker inspect --format='{{.State.Health.Status}}' monorepo_api
docker inspect --format='{{.State.Health.Status}}' monorepo_web

# Tail recent logs for the API
docker logs monorepo_api --tail 60

# Confirm all containers are running
docker compose ps
```

### Database Migration Failures

If the API container exits shortly after startup, a SQL migration likely failed:

```bash
docker logs monorepo_api 2>&1 | grep -i "migration\|error\|sqlx"
```

To reset the database volume and retry from scratch (all seeded data is lost):

```bash
docker compose down -v
docker compose up -d --build
```
