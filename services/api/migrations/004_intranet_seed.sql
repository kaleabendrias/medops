INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (
    SELECT 'auth.login' AS permission_key
    UNION ALL SELECT 'patient.read'
    UNION ALL SELECT 'patient.write'
    UNION ALL SELECT 'clinical.edit'
    UNION ALL SELECT 'bedboard.read'
    UNION ALL SELECT 'bedboard.write'
    UNION ALL SELECT 'dining.read'
    UNION ALL SELECT 'dining.write'
    UNION ALL SELECT 'order.write'
    UNION ALL SELECT 'governance.write'
    UNION ALL SELECT 'telemetry.write'
    UNION ALL SELECT 'audit.read'
    UNION ALL SELECT 'retention.manage'
    UNION ALL SELECT 'admin.disable_user'
) p
WHERE r.name = 'admin'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (
    SELECT 'patient.read' AS permission_key
    UNION ALL SELECT 'patient.write'
    UNION ALL SELECT 'clinical.edit'
    UNION ALL SELECT 'bedboard.read'
    UNION ALL SELECT 'bedboard.write'
    UNION ALL SELECT 'dining.read'
    UNION ALL SELECT 'order.write'
    UNION ALL SELECT 'telemetry.write'
) p
WHERE r.name = 'doctor'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (
    SELECT 'patient.read' AS permission_key
    UNION ALL SELECT 'clinical.edit'
    UNION ALL SELECT 'bedboard.read'
    UNION ALL SELECT 'bedboard.write'
    UNION ALL SELECT 'dining.read'
    UNION ALL SELECT 'order.write'
) p
WHERE r.name = 'nurse'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (
    SELECT 'audit.read' AS permission_key
    UNION ALL SELECT 'patient.read'
    UNION ALL SELECT 'bedboard.read'
    UNION ALL SELECT 'dining.read'
) p
WHERE r.name = 'auditor'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO menu_entitlements (role_id, menu_key, allowed)
SELECT r.id, m.menu_key, TRUE
FROM roles r
JOIN (
    SELECT 'dashboard' AS menu_key
    UNION ALL SELECT 'patients'
    UNION ALL SELECT 'clinical'
    UNION ALL SELECT 'bedboard'
    UNION ALL SELECT 'dining'
    UNION ALL SELECT 'orders'
    UNION ALL SELECT 'ingestion'
    UNION ALL SELECT 'telemetry'
    UNION ALL SELECT 'audits'
    UNION ALL SELECT 'admin'
) m
WHERE r.name = 'admin'
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);

INSERT INTO menu_entitlements (role_id, menu_key, allowed)
SELECT r.id, m.menu_key, TRUE
FROM roles r
JOIN (
    SELECT 'dashboard' AS menu_key
    UNION ALL SELECT 'patients'
    UNION ALL SELECT 'clinical'
    UNION ALL SELECT 'bedboard'
    UNION ALL SELECT 'dining'
    UNION ALL SELECT 'orders'
    UNION ALL SELECT 'telemetry'
) m
WHERE r.name IN ('doctor', 'nurse')
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);

INSERT INTO menu_entitlements (role_id, menu_key, allowed)
SELECT r.id, m.menu_key, TRUE
FROM roles r
JOIN (
    SELECT 'dashboard' AS menu_key
    UNION ALL SELECT 'patients'
    UNION ALL SELECT 'bedboard'
    UNION ALL SELECT 'dining'
    UNION ALL SELECT 'audits'
) m
WHERE r.name = 'auditor'
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);

INSERT INTO users (username, password_hash, role_id, is_disabled, failed_attempts, locked_until, last_activity_at, created_at, updated_at)
SELECT 'admin', '9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6', r.id, FALSE, 0, NULL, NOW(), NOW(), NOW()
FROM roles r
WHERE r.name = 'admin'
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id), password_hash = VALUES(password_hash), is_disabled = FALSE;

INSERT INTO buildings (code, name)
VALUES
    ('BLD-A', 'Main Hospital Building A'),
    ('BLD-B', 'Inpatient Building B')
ON DUPLICATE KEY UPDATE name = VALUES(name);

INSERT INTO units (building_id, code, name)
SELECT b.id, 'U-1', 'Acute Care Unit 1' FROM buildings b WHERE b.code = 'BLD-A'
ON DUPLICATE KEY UPDATE name = VALUES(name);

INSERT INTO units (building_id, code, name)
SELECT b.id, 'U-2', 'Recovery Unit 2' FROM buildings b WHERE b.code = 'BLD-B'
ON DUPLICATE KEY UPDATE name = VALUES(name);

INSERT INTO rooms (unit_id, code)
SELECT u.id, 'R-101' FROM units u WHERE u.code = 'U-1'
ON DUPLICATE KEY UPDATE code = VALUES(code);

INSERT INTO rooms (unit_id, code)
SELECT u.id, 'R-201' FROM units u WHERE u.code = 'U-2'
ON DUPLICATE KEY UPDATE code = VALUES(code);

INSERT INTO beds (room_id, bed_label, state)
SELECT r.id, 'A', 'Available' FROM rooms r WHERE r.code = 'R-101'
ON DUPLICATE KEY UPDATE state = VALUES(state);

INSERT INTO beds (room_id, bed_label, state)
SELECT r.id, 'B', 'Available' FROM rooms r WHERE r.code = 'R-101'
ON DUPLICATE KEY UPDATE state = VALUES(state);

INSERT INTO beds (room_id, bed_label, state)
SELECT r.id, 'A', 'Available' FROM rooms r WHERE r.code = 'R-201'
ON DUPLICATE KEY UPDATE state = VALUES(state);

INSERT INTO experiments (experiment_key, is_active)
VALUES ('intranet_ui_experiment', TRUE)
ON DUPLICATE KEY UPDATE is_active = VALUES(is_active);

INSERT INTO retention_policies (policy_key, years, updated_by, updated_at)
SELECT 'clinical_records', 7, u.id, NOW() FROM users u WHERE u.username = 'admin'
ON DUPLICATE KEY UPDATE years = VALUES(years), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at);
