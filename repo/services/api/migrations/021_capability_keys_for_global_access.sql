-- Replace hardcoded role-name shortcuts in the application layer with
-- permission-table-driven capability keys. Prior versions of the repository
-- branched on `matches!(role_name, "admin" | "auditor")` to decide whether
-- a request should bypass per-row ownership filters; this leaks the access
-- policy into source code and drifts from `role_permissions` over time.
--
-- The application layer now consults `role_permissions` for the three
-- capability keys below. This migration seeds those keys onto the same
-- roles that the legacy hardcoded check used to honour, so behaviour is
-- preserved while moving the source of truth into the database.
--
--   patient.global_access  — admin, auditor
--   ingestion.global_access — admin, auditor
--   order.global_access    — admin, auditor, employee
--                            (already seeded by migration 010, re-asserted
--                             here as an idempotent UPSERT for clarity)
--
-- Membership in `role_permissions` is the single source of truth for these
-- privileges. To grant or revoke global access in a future deployment,
-- modify `role_permissions` — never re-introduce a hardcoded role match.

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'patient.global_access' FROM roles r
WHERE r.name IN ('admin', 'auditor')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'ingestion.global_access' FROM roles r
WHERE r.name IN ('admin', 'auditor')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

-- Re-assert order.global_access (already seeded in migration 010) so a
-- repository deployment that loses the historical row still ends up with
-- the correct access set. Idempotent under the unique key.
INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'order.global_access' FROM roles r
WHERE r.name IN ('admin', 'auditor', 'employee')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);
