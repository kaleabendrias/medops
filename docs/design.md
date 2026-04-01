# MedOps Platform Design

## 1. Purpose
This document describes the implemented system design for the MedOps integrated facility platform in this repository. It reflects the current codebase architecture, runtime boundaries, and operational behavior.

## 2. System Context
The platform is an offline-first intranet stack for hospital operations with three primary user categories:
- Administrators
- Employees (clinical and operations roles)
- Members

Primary business domains implemented:
- Clinical profile management and revision timelines
- Bedboard and inpatient logistics state transitions
- Dining menus, orders, ticket splits, and operational notes
- Group-buy campaigns and inactivity auto-close logic
- Ingestion task management with versioning and rollback
- Governance, retention, telemetry, analytics, and audit visibility

## 3. High-Level Architecture

### 3.1 Runtime Components
- API service: Rust + Rocket
- Web service: Rust + Dioxus (web)
- Database: MySQL (single source of record)

### 3.2 Backend Layering
The API service is intentionally split into layers:
- contracts: request and response DTOs, API error mapping
- routes: HTTP handler endpoints
- services: use-case orchestration, policy and validation logic
- repositories: persistence traits
- infrastructure: MySQL adapters, auth middleware, crypto, migrations

### 3.3 Frontend Layering
The web client is split by responsibility:
- api: typed HTTP client wrappers for API endpoints
- components: shell, login gate, shared feedback
- features: page guards, navigation, clinical and order helpers
- state: session context, entitlement checks, page routing helpers
- ui_logic: upload and revision parsing helpers

## 4. Key Design Decisions

### 4.1 Offline and Local-First Operation
- Auth policy is configured as offline only.
- API and web are expected to run on local intranet addresses.
- Operational test pipeline is designed for deterministic local execution.

### 4.2 Authorization Model
- Session-based API authentication uses X-Session-Token.
- Menu-level entitlements drive frontend navigation visibility.
- API-level permission checks enforce server-side RBAC regardless of UI state.
- Object-level access is enforced in backend services and repository queries.

### 4.3 Sensitive Data Handling
- Sensitive clinical fields are encrypted at rest with application-level keys.
- Frontend and backend flows default to masked sensitive values unless privileged reveal applies.

### 4.4 Auditable Operational Flows
- Access, edits, exports, order actions, and admin actions are auditable.
- Governance supports tombstone workflows.

## 5. Core Workflow Design

### 5.1 Authentication and Session
1. User submits username/password via login gate.
2. API validates credential policy and account state.
3. Frontend fetches menu entitlements after login.
4. Session context and allowed pages are resolved.
5. Unauthorized pages are redirected to an accessible page.

### 5.2 Clinical and Patient Workflows
1. Search and select patient.
2. Load profile, revisions, and attachments.
3. Require reason_for_change for demographics and clinical edits.
4. Render revision deltas with masked or revealed values by role.
5. Upload/download attachments with file type and size constraints.

### 5.3 Bedboard Operations
1. Poll bed and event data periodically.
2. Execute transitions with action, target state, and optional related bed.
3. Record actor and timeline entries for operational history.

### 5.4 Dining and Orders
1. Manage dishes, options, windows, publication, and sold-out state.
2. Place and list orders.
3. Apply state transitions with reason required for Canceled and Credited.
4. Manage ticket splits and staff operation notes.

### 5.5 Campaigns
1. Create campaign with threshold and deadline.
2. Allow users to join campaign.
3. Backend updates campaign status on qualifying activity.
4. Backend closes inactive campaigns after configured inactivity logic.

### 5.6 Ingestion and Analytics
1. Create and update ingestion tasks.
2. Run tasks, inspect versions and runs.
3. Roll back task versions with required reason.
4. Consume funnel, retention, and recommendation KPI analytics endpoints.

## 6. Reliability and Validation Strategy
- Frontend input validation for key workflows (examples: uploads, order transitions, ID parsing).
- Backend validation and permission checks remain authoritative.
- Action feedback is surfaced through status and error banners.
- Polling is used for bedboard freshness.

## 7. Deployment and Verification Boundary
- Primary integrated run path uses Docker Compose for mysql, api, and web.
- Non-Docker frontend checks are available for local Rust toolchains.

## 8. Known Constraints and Trade-Offs
- Integrated verification is Docker-first for deterministic seeded data and end-to-end checks.
- Intranet assumptions are reflected in base URLs and allowed origins configuration.
- Frontend is feature-complete but heavily concentrated in a single large main view module, which may increase maintenance cost.

## 9. Future Improvements
- Further decompose frontend page modules and shared state hooks.
- Expand UI-level E2E tests that drive the actual web application.
- Add explicit frontend logout revocation flow if server logout endpoint is introduced.
