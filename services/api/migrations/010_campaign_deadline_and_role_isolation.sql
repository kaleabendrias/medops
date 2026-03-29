SET @has_deadline_col := (
    SELECT COUNT(*)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'group_campaigns'
      AND column_name = 'success_deadline_at'
);
SET @add_deadline_sql := IF(
    @has_deadline_col = 0,
    'ALTER TABLE group_campaigns ADD COLUMN success_deadline_at DATETIME NULL AFTER success_threshold',
    'SELECT 1'
);
PREPARE stmt_add_deadline FROM @add_deadline_sql;
EXECUTE stmt_add_deadline;
DEALLOCATE PREPARE stmt_add_deadline;

UPDATE group_campaigns
SET success_deadline_at = DATE_ADD(created_at, INTERVAL 1 HOUR)
WHERE success_deadline_at IS NULL;

ALTER TABLE group_campaigns
    MODIFY COLUMN success_deadline_at DATETIME NOT NULL;

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'catalog.read' FROM roles r
WHERE r.name IN ('admin', 'doctor', 'nurse', 'auditor', 'employee')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'order.global_access' FROM roles r
WHERE r.name IN ('admin', 'auditor', 'employee')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

DELETE rp FROM role_permissions rp
JOIN roles r ON r.id = rp.role_id
WHERE r.name = 'employee'
  AND rp.permission_key IN ('patient.read', 'patient.write', 'clinical.edit', 'bedboard.read', 'bedboard.write');

DELETE me FROM menu_entitlements me
JOIN roles r ON r.id = me.role_id
WHERE r.name = 'employee'
  AND me.menu_key IN ('patients', 'clinical', 'bedboard');

DELETE me FROM menu_entitlements me
JOIN roles r ON r.id = me.role_id
WHERE r.name IN ('doctor', 'nurse')
  AND me.menu_key IN ('dining', 'campaigns');

INSERT INTO users (username, password_hash, role_id, is_disabled, failed_attempts, locked_until, last_activity_at, created_at, updated_at)
SELECT 'clinical1', '9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6', r.id, FALSE, 0, NULL, NOW(), NOW(), NOW()
FROM roles r
WHERE r.name = 'doctor'
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id), password_hash = VALUES(password_hash), is_disabled = FALSE;

INSERT INTO users (username, password_hash, role_id, is_disabled, failed_attempts, locked_until, last_activity_at, created_at, updated_at)
SELECT 'cafeteria1', '9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6', r.id, FALSE, 0, NULL, NOW(), NOW(), NOW()
FROM roles r
WHERE r.name = 'employee'
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id), password_hash = VALUES(password_hash), is_disabled = FALSE;
