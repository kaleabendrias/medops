# MedOps API Specification

## 1. Overview
This document describes the implemented intranet REST API consumed by the Dioxus web application.

Base namespace:
- /api/v1

Primary runtime endpoint defaults:
- API host: http://localhost:8000 (internal; not exposed to end-users)
- Web host: https://localhost:8443

## 2. Authentication and Session

### 2.1 Login
- Method: POST
- Path: /api/v1/auth/login
- Request body: AuthLoginRequest
	- username: string
	- password: string
- Success response: AuthLoginResponse
	- csrf_token: string — derived as hex(SHA256(bearer_token + ":csrf-v1")); must be sent as X-CSRF-Token on mutating requests
	- user_id: integer
	- username: string
	- role: string
	- expires_in_minutes: integer
- Side effect: sets an HttpOnly, Secure cookie named `hospital_session` containing the session bearer token. The browser sends this cookie automatically on subsequent same-origin requests.

### 2.2 Logout
- Method: POST
- Path: /api/v1/auth/logout
- Requires: `hospital_session` cookie and `X-CSRF-Token` header
- Revokes the active session server-side and clears the cookie.

### 2.3 Authenticated Requests
- Session credential: HttpOnly cookie `hospital_session` set at login — sent automatically by the browser; no Authorization header is used.
- CSRF protection: POST, PUT, DELETE, and PATCH requests must include the header `X-CSRF-Token: <csrf_token>` where `csrf_token` was returned by the login response. GET, HEAD, and OPTIONS requests are exempt.
- All protected endpoints require a valid `hospital_session` cookie except:
	- GET /api/v1/health
	- POST /api/v1/auth/login
	- POST /api/v1/auth/logout

## 3. Global Behavior

### 3.1 Content Types
- JSON endpoints use application/json.
- Attachment upload endpoint accepts binary payload.

### 3.2 Error Semantics
Typical status classes used across endpoints:
- 200 for success with payload
- 400 for validation or policy errors
- 401 for missing or invalid session
- 403 for permission or entitlement denial
- 404 for missing resources
- 409 for version or transition conflicts where applicable
- 413 for oversized attachment payloads

### 3.3 Health
- Method: GET
- Path: /api/v1/health
- Response: HealthResponse with service and database connectivity status.

## 4. Endpoint Catalog by Domain

## 4.1 Catalog and RBAC
- GET /api/v1/hospitals
- GET /api/v1/roles
- GET /api/v1/rbac/menu-entitlements
- GET /api/v1/admin/users
- POST /api/v1/admin/users/{user_id}/disable

## 4.2 Patients and Clinical
- POST /api/v1/patients
- GET /api/v1/patients
- GET /api/v1/patients/search?q={query}
- GET /api/v1/patients/{patient_id}
- POST /api/v1/patients/{patient_id}/assign
- PUT /api/v1/patients/{patient_id}
- PUT /api/v1/patients/{patient_id}/allergies
- PUT /api/v1/patients/{patient_id}/contraindications
- PUT /api/v1/patients/{patient_id}/history
- POST /api/v1/patients/{patient_id}/visit-notes
- GET /api/v1/patients/{patient_id}/revisions

### 4.2.1 Attachment Operations
- POST /api/v1/patients/{patient_id}/attachments?filename={name}&mime_type={mime}
- GET /api/v1/patients/{patient_id}/attachments
- GET /api/v1/patients/{patient_id}/attachments/{attachment_id}/download

### 4.2.2 Export
- GET /api/v1/patients/{patient_id}/export?format={json|csv}&reveal_sensitive={true|false}

## 4.3 Bedboard
- GET /api/v1/bedboard/beds
- POST /api/v1/bedboard/beds/{bed_id}/transition
- GET /api/v1/bedboard/events

## 4.4 Dining and Orders
- POST /api/v1/dining/menus
- GET /api/v1/dining/menus
- POST /api/v1/orders
- PUT /api/v1/orders/{order_id}/status
- GET /api/v1/orders
- POST /api/v1/orders/{order_id}/ticket-splits
- GET /api/v1/orders/{order_id}/ticket-splits
- POST /api/v1/orders/{order_id}/notes
- GET /api/v1/orders/{order_id}/notes

## 4.5 Cafeteria Management
- GET /api/v1/cafeteria/categories
- POST /api/v1/cafeteria/dishes
- GET /api/v1/cafeteria/dishes
- PUT /api/v1/cafeteria/dishes/{dish_id}/status
- POST /api/v1/cafeteria/dishes/{dish_id}/options
- POST /api/v1/cafeteria/dishes/{dish_id}/windows
- PUT /api/v1/cafeteria/ranking-rules
- GET /api/v1/cafeteria/ranking-rules
- GET /api/v1/cafeteria/recommendations

## 4.6 Campaigns
- POST /api/v1/campaigns
- POST /api/v1/campaigns/{campaign_id}/join
- GET /api/v1/campaigns

## 4.7 Experimentation and Analytics
- POST /api/v1/experiments
- POST /api/v1/experiments/{experiment_id}/variants
- POST /api/v1/experiments/{experiment_id}/assign
- POST /api/v1/experiments/{experiment_id}/backtrack
- GET /api/v1/analytics/funnel
- GET /api/v1/analytics/retention
- GET /api/v1/analytics/recommendation-kpi
- POST /api/v1/telemetry/events

## 4.8 Governance and Audit
- POST /api/v1/governance/records
- GET /api/v1/governance/records
- DELETE /api/v1/governance/records/{record_id}
- GET /api/v1/audits

## 4.9 Ingestion
- POST /api/v1/ingestion/tasks
- PUT /api/v1/ingestion/tasks/{task_id}
- POST /api/v1/ingestion/tasks/{task_id}/rollback
- POST /api/v1/ingestion/tasks/{task_id}/run
- GET /api/v1/ingestion/tasks
- GET /api/v1/ingestion/tasks/{task_id}/versions
- GET /api/v1/ingestion/tasks/{task_id}/runs

## 4.10 Retention and Session Settings
- GET /api/v1/retention/settings
- GET /api/v1/retention/policies
- PUT /api/v1/retention/policies/{policy_key}/{years}
- GET /api/v1/session

## 5. Representative Request and Response Contracts

### 5.1 Clinical Edit Request
- value: string
- reason_for_change: string (required)

### 5.2 Bed Transition Request
- action: string
- target_state: string
- related_bed_id: integer or null
- note: string

### 5.3 Order Status Request
- status: Created | Billed | Canceled | Credited
- reason: optional string (required by policy for selected transitions)
- expected_version: optional integer for optimistic concurrency

### 5.4 Ingestion Create Request
- task_name: string
- seed_urls: array of strings
- extraction_rules_json: string
- pagination_strategy: string
- max_depth: integer
- incremental_field: optional string
- schedule_cron: string

## 6. Security and Policy Notes
- Password policy, lockout window, inactivity timeout, and offline mode are enforced server-side.
- RBAC and object-level checks are server-authoritative; frontend menu entitlements are supplementary UX controls.
- Sensitive clinical data masking and reveal permissions are role-aware.

## 7. Versioning and Compatibility
- Namespace versioning is path-based with /api/v1.
- Changes that break request or response contracts should advance API version or include compatibility adapters.
