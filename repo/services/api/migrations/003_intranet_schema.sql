CREATE TABLE IF NOT EXISTS role_permissions (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    role_id BIGINT NOT NULL,
    permission_key VARCHAR(128) NOT NULL,
    UNIQUE KEY uniq_role_perm (role_id, permission_key),
    CONSTRAINT fk_role_perm_role FOREIGN KEY (role_id) REFERENCES roles(id)
);

CREATE TABLE IF NOT EXISTS menu_entitlements (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    role_id BIGINT NOT NULL,
    menu_key VARCHAR(128) NOT NULL,
    allowed BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE KEY uniq_role_menu (role_id, menu_key),
    CONSTRAINT fk_menu_role FOREIGN KEY (role_id) REFERENCES roles(id)
);

CREATE TABLE IF NOT EXISTS users (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(128) NOT NULL,
    role_id BIGINT NOT NULL,
    is_disabled BOOLEAN NOT NULL DEFAULT FALSE,
    failed_attempts INT NOT NULL DEFAULT 0,
    locked_until DATETIME NULL,
    last_activity_at DATETIME NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_users_role FOREIGN KEY (role_id) REFERENCES roles(id)
);

CREATE TABLE IF NOT EXISTS sessions (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    session_token VARCHAR(128) NOT NULL UNIQUE,
    user_id BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    last_activity_at DATETIME NOT NULL,
    revoked_at DATETIME NULL,
    CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS patients (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    mrn VARCHAR(64) NOT NULL UNIQUE,
    first_name VARCHAR(120) NOT NULL,
    last_name VARCHAR(120) NOT NULL,
    birth_date VARCHAR(16) NOT NULL,
    gender VARCHAR(32) NOT NULL,
    phone VARCHAR(64) NOT NULL,
    email VARCHAR(255) NOT NULL,
    allergies TEXT NOT NULL,
    contraindications TEXT NOT NULL,
    history TEXT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS patient_visit_notes (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    patient_id BIGINT NOT NULL,
    note TEXT NOT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_visit_note_patient FOREIGN KEY (patient_id) REFERENCES patients(id),
    CONSTRAINT fk_visit_note_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS patient_revisions (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    patient_id BIGINT NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    diff_before TEXT NOT NULL,
    diff_after TEXT NOT NULL,
    reason_for_change TEXT NOT NULL,
    actor_id BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_patient_rev_patient FOREIGN KEY (patient_id) REFERENCES patients(id),
    CONSTRAINT fk_patient_rev_actor FOREIGN KEY (actor_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS patient_attachments (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    patient_id BIGINT NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    mime_type VARCHAR(64) NOT NULL,
    file_size_bytes BIGINT NOT NULL,
    storage_path VARCHAR(512) NOT NULL,
    uploaded_by BIGINT NOT NULL,
    uploaded_at DATETIME NOT NULL,
    CONSTRAINT fk_attach_patient FOREIGN KEY (patient_id) REFERENCES patients(id),
    CONSTRAINT fk_attach_user FOREIGN KEY (uploaded_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS buildings (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    code VARCHAR(32) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL
);

CREATE TABLE IF NOT EXISTS units (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    building_id BIGINT NOT NULL,
    code VARCHAR(32) NOT NULL,
    name VARCHAR(255) NOT NULL,
    UNIQUE KEY uniq_unit_code (building_id, code),
    CONSTRAINT fk_units_building FOREIGN KEY (building_id) REFERENCES buildings(id)
);

CREATE TABLE IF NOT EXISTS rooms (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    unit_id BIGINT NOT NULL,
    code VARCHAR(32) NOT NULL,
    UNIQUE KEY uniq_room_code (unit_id, code),
    CONSTRAINT fk_rooms_unit FOREIGN KEY (unit_id) REFERENCES units(id)
);

CREATE TABLE IF NOT EXISTS beds (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    room_id BIGINT NOT NULL,
    bed_label VARCHAR(32) NOT NULL,
    state VARCHAR(32) NOT NULL,
    UNIQUE KEY uniq_bed_label (room_id, bed_label),
    CONSTRAINT fk_beds_room FOREIGN KEY (room_id) REFERENCES rooms(id)
);

CREATE TABLE IF NOT EXISTS bed_events (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    action_type VARCHAR(32) NOT NULL,
    from_bed_id BIGINT NULL,
    to_bed_id BIGINT NULL,
    from_state VARCHAR(32) NULL,
    to_state VARCHAR(32) NULL,
    actor_id BIGINT NOT NULL,
    note TEXT NOT NULL,
    occurred_at DATETIME NOT NULL,
    CONSTRAINT fk_bed_event_actor FOREIGN KEY (actor_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS dining_menus (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    menu_date VARCHAR(16) NOT NULL,
    meal_period VARCHAR(32) NOT NULL,
    item_name VARCHAR(255) NOT NULL,
    calories INT NOT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_menu_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS dining_orders (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    patient_id BIGINT NOT NULL,
    menu_id BIGINT NOT NULL,
    status VARCHAR(32) NOT NULL,
    notes TEXT NOT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_order_patient FOREIGN KEY (patient_id) REFERENCES patients(id),
    CONSTRAINT fk_order_menu FOREIGN KEY (menu_id) REFERENCES dining_menus(id),
    CONSTRAINT fk_order_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS governance_records (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    tier VARCHAR(32) NOT NULL,
    lineage_source_id BIGINT NULL,
    lineage_metadata TEXT NOT NULL,
    payload_json LONGTEXT NOT NULL,
    tombstoned BOOLEAN NOT NULL DEFAULT FALSE,
    tombstone_reason TEXT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_governance_source FOREIGN KEY (lineage_source_id) REFERENCES governance_records(id),
    CONSTRAINT fk_governance_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS experiments (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    experiment_key VARCHAR(128) NOT NULL UNIQUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS telemetry_events (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    experiment_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    event_name VARCHAR(128) NOT NULL,
    payload_json LONGTEXT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_tel_exp FOREIGN KEY (experiment_id) REFERENCES experiments(id),
    CONSTRAINT fk_tel_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS retention_policies (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    policy_key VARCHAR(128) NOT NULL UNIQUE,
    years INT NOT NULL,
    updated_by BIGINT NOT NULL,
    updated_at DATETIME NOT NULL,
    CONSTRAINT fk_retention_user FOREIGN KEY (updated_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    action_type VARCHAR(64) NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id VARCHAR(128) NOT NULL,
    details_json LONGTEXT NOT NULL,
    actor_id BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_audit_user FOREIGN KEY (actor_id) REFERENCES users(id)
);
