# Questions and Clarifications

This file records implementation-time clarifications and decisions as reflected in the current codebase.

## Q1: What is the authoritative runtime path for full verification?
- Question:
Is full-stack verification expected to run natively or containerized?
- Context:
The stack has API, web, seeded data, migrations, and cross-role integration checks.
- Resolution:
Containerized verification is authoritative. Non-Docker frontend checks are supported as a secondary path for local frontend iteration.

## Q2: What is the session and authentication model?
- Question:
How should authentication be enforced for an offline intranet deployment?
- Context:
The prompt requires username/password, lockout policy, and inactivity expiry.
- Resolution:
Session token based authentication is used with X-Session-Token header. Offline-only policy is enforced in configuration and startup checks. Password policy, lockout thresholds, and inactivity timeout are server-configured.

## Q3: Where is authorization enforced?
- Question:
Should role checks happen only in frontend navigation or also in API services?
- Context:
Frontend can hide routes, but secure enforcement requires backend checks.
- Resolution:
Authorization is layered: frontend menu entitlements govern page visibility, while backend permission and object-level checks remain authoritative.

## Q4: How are sensitive clinical fields handled?
- Question:
How should allergy/contraindication/history and identifier-like fields be protected?
- Context:
Prompt requires encrypted-at-rest handling and masked display by default.
- Resolution:
Application-level encryption is used server-side for sensitive fields with role-aware reveal controls. Frontend defaults to masked sensitive values unless reveal is permitted.

## Q5: Are reason-for-change semantics mandatory for clinical edits?
- Question:
Do demographics and clinical updates require a non-empty change reason?
- Context:
Clinical traceability requires revision context.
- Resolution:
Reason-for-change is treated as required in update workflows and revision timeline outputs.

## Q6: How should attachment uploads be constrained?
- Question:
What validations are mandatory for patient attachment uploads?
- Context:
Prompt specifies file types and per-file size constraints.
- Resolution:
Only PDF/JPG/PNG with matching MIME types are accepted, and per-file size is limited to 25 MB.

## Q7: What bedboard lifecycle model is supported?
- Question:
Should bed transitions be treated as free-form updates or constrained state-machine actions?
- Context:
Operational integrity requires legal transition checks and auditability.
- Resolution:
Bed transitions are validated by legal state rules and recorded as operational events with actor and timeline details.

## Q8: How are campaign completion and closure handled?
- Question:
How does the platform determine campaign success versus closure due to inactivity or deadline?
- Context:
Prompt requires threshold-based success and auto-close inactivity behavior.
- Resolution:
Campaign status is recomputed by backend logic using qualifying orders, deadlines, and inactivity closure conditions.

## Q9: What is the intended behavior for ingestion task lifecycle?
- Question:
Are ingestion tasks one-shot jobs or managed assets with versions and rollback?
- Context:
Prompt calls for strategy configuration, scheduling, and rollback.
- Resolution:
Tasks are managed entities with create, update, run, version history, and rollback operations.

## Q10: What is the testing expectation for acceptance confidence?
- Question:
Which test layers are expected for delivery confidence?
- Context:
The stack includes backend, frontend, integration, authorization matrix, browser checks, and smoke tests.
- Resolution:
Acceptance confidence is based on multi-layer checks: unit tests, API integration, role matrix validation, browser checks, and smoke workflow scripts.

## Notes
- This document reflects currently implemented behavior and may be updated when requirements or policies change.
