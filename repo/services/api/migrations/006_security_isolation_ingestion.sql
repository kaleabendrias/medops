ALTER TABLE patients
    ADD COLUMN created_by BIGINT NULL,
    ADD COLUMN assigned_team VARCHAR(64) NULL,
    ADD COLUMN ssn_cipher TEXT NULL,
    ADD COLUMN mrn_cipher TEXT NULL,
    ADD COLUMN mrn_hash CHAR(64) NULL,
    ADD COLUMN allergies_cipher LONGTEXT NULL,
    ADD COLUMN contraindications_cipher LONGTEXT NULL,
    ADD COLUMN history_cipher LONGTEXT NULL,
    ADD COLUMN encryption_key_version VARCHAR(32) NOT NULL DEFAULT 'v1',
    ADD COLUMN last_sensitive_reveal_at DATETIME NULL;

ALTER TABLE patients
    ADD CONSTRAINT fk_patients_created_by_user FOREIGN KEY (created_by) REFERENCES users(id);

CREATE INDEX idx_patients_created_by ON patients(created_by);
CREATE INDEX idx_patients_mrn_hash ON patients(mrn_hash);

UPDATE patients p
JOIN users u ON u.username = 'admin'
SET p.created_by = u.id
WHERE p.created_by IS NULL;

CREATE TABLE IF NOT EXISTS patient_assignments (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    patient_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    assignment_type VARCHAR(32) NOT NULL,
    assigned_by BIGINT NOT NULL,
    assigned_at DATETIME NOT NULL,
    UNIQUE KEY uniq_patient_user_assignment (patient_id, user_id),
    CONSTRAINT fk_patient_assignment_patient FOREIGN KEY (patient_id) REFERENCES patients(id),
    CONSTRAINT fk_patient_assignment_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_patient_assignment_actor FOREIGN KEY (assigned_by) REFERENCES users(id)
);

INSERT INTO patient_assignments (patient_id, user_id, assignment_type, assigned_by, assigned_at)
SELECT p.id, p.created_by, 'owner', p.created_by, NOW()
FROM patients p
WHERE p.created_by IS NOT NULL
ON DUPLICATE KEY UPDATE assignment_type = VALUES(assignment_type);

ALTER TABLE dining_orders
    ADD COLUMN idempotency_key VARCHAR(128) NULL,
    ADD COLUMN billed_at DATETIME NULL,
    ADD COLUMN canceled_at DATETIME NULL,
    ADD COLUMN credited_at DATETIME NULL,
    ADD COLUMN status_reason TEXT NULL,
    ADD COLUMN version INT NOT NULL DEFAULT 0;

CREATE UNIQUE INDEX uniq_order_idempotency_key ON dining_orders(idempotency_key);
CREATE INDEX idx_orders_created_by ON dining_orders(created_by);
CREATE INDEX idx_orders_patient_status ON dining_orders(patient_id, status);

UPDATE dining_orders
SET status = CASE
    WHEN status IN ('Placed', 'Approved', 'Preparing') THEN 'Created'
    WHEN status = 'Served' THEN 'Billed'
    WHEN status = 'Cancelled' THEN 'Canceled'
    WHEN status = 'Credited' THEN 'Credited'
    ELSE status
END;

ALTER TABLE audit_logs
    ADD COLUMN event_seq BIGINT NULL,
    ADD COLUMN prev_hash CHAR(64) NULL,
    ADD COLUMN entry_hash CHAR(64) NULL;

CREATE UNIQUE INDEX uniq_audit_event_seq ON audit_logs(event_seq);
CREATE INDEX idx_audit_entry_hash ON audit_logs(entry_hash);

CREATE TABLE IF NOT EXISTS ingestion_tasks (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    task_name VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL,
    active_version BIGINT NULL,
    schedule_cron VARCHAR(128) NOT NULL,
    max_depth INT NOT NULL,
    pagination_strategy VARCHAR(64) NOT NULL,
    incremental_field VARCHAR(128) NULL,
    next_run_at DATETIME NULL,
    last_run_at DATETIME NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    UNIQUE KEY uniq_ingestion_task_name (task_name),
    CONSTRAINT fk_ingestion_task_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS ingestion_task_versions (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    task_id BIGINT NOT NULL,
    version_number INT NOT NULL,
    seed_urls_json LONGTEXT NOT NULL,
    extraction_rules_json LONGTEXT NOT NULL,
    rollback_of_version INT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    UNIQUE KEY uniq_ingestion_task_version (task_id, version_number),
    CONSTRAINT fk_ingestion_task_version_task FOREIGN KEY (task_id) REFERENCES ingestion_tasks(id),
    CONSTRAINT fk_ingestion_task_version_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS ingestion_task_runs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    task_id BIGINT NOT NULL,
    task_version INT NOT NULL,
    status VARCHAR(32) NOT NULL,
    started_at DATETIME NOT NULL,
    finished_at DATETIME NULL,
    records_extracted INT NOT NULL DEFAULT 0,
    diagnostics_json LONGTEXT NOT NULL,
    CONSTRAINT fk_ingestion_run_task FOREIGN KEY (task_id) REFERENCES ingestion_tasks(id)
);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'order.read' FROM roles r
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'patient.reveal_sensitive' FROM roles r WHERE r.name IN ('admin', 'doctor', 'nurse')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'ingestion.read' FROM roles r WHERE r.name IN ('admin', 'auditor')
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'ingestion.manage' FROM roles r WHERE r.name = 'admin'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO menu_entitlements (role_id, menu_key, allowed)
SELECT r.id, 'ingestion', TRUE FROM roles r WHERE r.name = 'admin'
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);

INSERT INTO users (username, password_hash, role_id, is_disabled, failed_attempts, locked_until, last_activity_at, created_at, updated_at)
SELECT 'lockout_user', '9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6', r.id, FALSE, 0, NULL, NOW(), NOW(), NOW()
FROM roles r
WHERE r.name = 'employee'
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id), password_hash = VALUES(password_hash), is_disabled = FALSE;
