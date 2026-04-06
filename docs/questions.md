# Required Document Description: Business Logic Questions Log

## 1. Authoritative runtime path for full verification
Question: Is full-stack verification expected to run natively or containerized?

My Understanding: Containerized verification should be the authoritative path for full-system acceptance, while non-Docker frontend checks remain useful for local iteration.

Solution: Defined Docker-based verification as primary and retained native frontend checks as a secondary developer workflow.

## 2. Session and authentication model for offline intranet
Question: How should authentication be enforced for an offline intranet deployment?

My Understanding: Authentication must enforce session security plus policy controls (password complexity, lockout, inactivity timeout) under an offline-only operating mode.

Solution: Implemented session-token authentication via the X-Session-Token header, with startup/config enforcement for offline mode, lockout thresholds, password rules, and inactivity expiry.

## 3. Authorization enforcement location
Question: Should role checks happen only in frontend navigation or also in API services?

My Understanding: Frontend visibility controls are not sufficient for security; backend authorization and object-level checks must be authoritative.

Solution: Applied layered authorization where menu entitlements control page visibility and backend permission/object checks enforce access.

## 4. Sensitive clinical field protection
Question: How should allergy/contraindication/history and identifier-like fields be protected?

My Understanding: Sensitive fields require encrypted-at-rest treatment and masked-by-default display, with limited reveal capability by role.

Solution: Added server-side application encryption for sensitive data and role-aware reveal controls, while keeping frontend default rendering masked.

## 5. Reason-for-change requirement for clinical edits
Question: Do demographics and clinical updates require a non-empty change reason?

My Understanding: Clinical traceability requires a mandatory reason to support revision integrity and audit context.

Solution: Enforced reason-for-change in update workflows and exposed it in revision timeline outputs.

## 6. Attachment upload constraints
Question: What validations are mandatory for patient attachment uploads?

My Understanding: Upload validation must enforce both file type/MIME correctness and a strict per-file size limit.

Solution: Restricted uploads to PDF/JPG/PNG with matching MIME types and capped each file at 25 MB.

## 7. Bedboard lifecycle model
Question: Should bed transitions be treated as free-form updates or constrained state-machine actions?

My Understanding: Bed movement should follow legal transition rules to preserve operational integrity and auditable history.

Solution: Implemented legal-state validation for transitions and recorded bed events with actor and timeline details.

## 8. Campaign completion and closure logic
Question: How does the platform determine campaign success versus closure due to inactivity or deadline?

My Understanding: Campaign outcomes should be backend-derived using qualifying order totals, configured thresholds, inactivity windows, and deadlines.

Solution: Implemented status recomputation logic that evaluates success conditions and auto-closes campaigns when inactivity/deadline criteria are met.

# 9. Ingestion task lifecycle behavior
Question: Are ingestion tasks one-shot jobs or managed assets with versions and rollback?

My Understanding: Ingestion should be a managed lifecycle with durable history, repeatable execution, and rollback support.

Solution: Implemented task entities with create/update/run flows plus version history and rollback operations.

# 10. Testing expectations for acceptance confidence
Question: Which test layers are expected for delivery confidence?

My Understanding: Acceptance confidence requires multiple complementary layers, not only unit testing.

Solution: Adopted multi-layer coverage through backend/frontend unit tests, API integration tests, role authorization matrix checks, browser checks, and smoke workflows.
