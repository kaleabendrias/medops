DROP TRIGGER IF EXISTS trg_audit_logs_block_update;
DROP TRIGGER IF EXISTS trg_audit_logs_block_delete;

CREATE TRIGGER trg_audit_logs_block_update
BEFORE UPDATE ON audit_logs
FOR EACH ROW
SIGNAL SQLSTATE '45000'
SET MESSAGE_TEXT = 'audit_logs is append-only; UPDATE rejected';

CREATE TRIGGER trg_audit_logs_block_delete
BEFORE DELETE ON audit_logs
FOR EACH ROW
SIGNAL SQLSTATE '45000'
SET MESSAGE_TEXT = 'audit_logs is append-only; DELETE rejected';

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'patient.export' FROM roles r
WHERE r.name IN ('admin', 'doctor', 'nurse', 'auditor')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);
