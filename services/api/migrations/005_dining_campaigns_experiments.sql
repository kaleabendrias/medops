INSERT INTO roles (name, description)
VALUES
  ('employee', 'General staff workflows for intranet operations'),
  ('member', 'Patient/member self-service ordering and campaigns')
ON DUPLICATE KEY UPDATE description = VALUES(description);

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
WHERE r.name = 'employee'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (
    SELECT 'dining.read' AS permission_key
    UNION ALL SELECT 'order.write'
    UNION ALL SELECT 'telemetry.write'
) p
WHERE r.name = 'member'
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
    UNION ALL SELECT 'campaigns'
) m
WHERE r.name = 'employee'
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);

INSERT INTO menu_entitlements (role_id, menu_key, allowed)
SELECT r.id, m.menu_key, TRUE
FROM roles r
JOIN (
    SELECT 'dashboard' AS menu_key
    UNION ALL SELECT 'dining'
    UNION ALL SELECT 'orders'
    UNION ALL SELECT 'campaigns'
) m
WHERE r.name = 'member'
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);

INSERT INTO users (username, password_hash, role_id, is_disabled, failed_attempts, locked_until, last_activity_at, created_at, updated_at)
SELECT 'employee1', '9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6', r.id, FALSE, 0, NULL, NOW(), NOW(), NOW()
FROM roles r
WHERE r.name = 'employee'
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id), password_hash = VALUES(password_hash), is_disabled = FALSE;

INSERT INTO users (username, password_hash, role_id, is_disabled, failed_attempts, locked_until, last_activity_at, created_at, updated_at)
SELECT 'member1', '9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6', r.id, FALSE, 0, NULL, NOW(), NOW(), NOW()
FROM roles r
WHERE r.name = 'member'
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id), password_hash = VALUES(password_hash), is_disabled = FALSE;

CREATE TABLE IF NOT EXISTS dish_categories (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(120) NOT NULL UNIQUE,
    created_at DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS dishes (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    category_id BIGINT NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    base_price_cents INT NOT NULL,
    photo_path VARCHAR(512) NOT NULL,
    is_published BOOLEAN NOT NULL DEFAULT FALSE,
    is_sold_out BOOLEAN NOT NULL DEFAULT FALSE,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    CONSTRAINT fk_dish_cat FOREIGN KEY (category_id) REFERENCES dish_categories(id),
    CONSTRAINT fk_dish_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS dish_options (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    dish_id BIGINT NOT NULL,
    option_group VARCHAR(64) NOT NULL,
    option_value VARCHAR(128) NOT NULL,
    delta_price_cents INT NOT NULL,
    CONSTRAINT fk_dish_opt_dish FOREIGN KEY (dish_id) REFERENCES dishes(id)
);

CREATE TABLE IF NOT EXISTS dish_sales_windows (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    dish_id BIGINT NOT NULL,
    slot_name VARCHAR(64) NOT NULL,
    start_hhmm VARCHAR(5) NOT NULL,
    end_hhmm VARCHAR(5) NOT NULL,
    CONSTRAINT fk_dish_window_dish FOREIGN KEY (dish_id) REFERENCES dishes(id)
);

CREATE TABLE IF NOT EXISTS ranking_rules (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    rule_key VARCHAR(128) NOT NULL UNIQUE,
    weight DOUBLE NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by BIGINT NOT NULL,
    updated_at DATETIME NOT NULL,
    CONSTRAINT fk_rank_user FOREIGN KEY (updated_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS group_campaigns (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    title VARCHAR(255) NOT NULL,
    dish_id BIGINT NOT NULL,
    success_threshold INT NOT NULL,
    status VARCHAR(32) NOT NULL,
    created_by BIGINT NOT NULL,
    last_activity_at DATETIME NOT NULL,
    created_at DATETIME NOT NULL,
    closed_at DATETIME NULL,
    CONSTRAINT fk_campaign_dish FOREIGN KEY (dish_id) REFERENCES dishes(id),
    CONSTRAINT fk_campaign_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS campaign_members (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    campaign_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    joined_at DATETIME NOT NULL,
    UNIQUE KEY uniq_campaign_member (campaign_id, user_id),
    CONSTRAINT fk_camp_mem_campaign FOREIGN KEY (campaign_id) REFERENCES group_campaigns(id),
    CONSTRAINT fk_camp_mem_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS order_tickets (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_id BIGINT NOT NULL,
    split_by VARCHAR(32) NOT NULL,
    split_value VARCHAR(120) NOT NULL,
    quantity INT NOT NULL,
    CONSTRAINT fk_ticket_order FOREIGN KEY (order_id) REFERENCES dining_orders(id)
);

CREATE TABLE IF NOT EXISTS order_operation_notes (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_id BIGINT NOT NULL,
    note TEXT NOT NULL,
    staff_user_id BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_order_note_order FOREIGN KEY (order_id) REFERENCES dining_orders(id),
    CONSTRAINT fk_order_note_user FOREIGN KEY (staff_user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS experiment_variants (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    experiment_id BIGINT NOT NULL,
    variant_key VARCHAR(120) NOT NULL,
    allocation_weight DOUBLE NOT NULL,
    feature_version VARCHAR(64) NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE KEY uniq_exp_variant (experiment_id, variant_key),
    CONSTRAINT fk_variant_exp FOREIGN KEY (experiment_id) REFERENCES experiments(id)
);

CREATE TABLE IF NOT EXISTS experiment_assignments (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    experiment_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    variant_id BIGINT NOT NULL,
    assigned_at DATETIME NOT NULL,
    UNIQUE KEY uniq_exp_user (experiment_id, user_id),
    CONSTRAINT fk_assign_exp FOREIGN KEY (experiment_id) REFERENCES experiments(id),
    CONSTRAINT fk_assign_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_assign_variant FOREIGN KEY (variant_id) REFERENCES experiment_variants(id)
);

CREATE TABLE IF NOT EXISTS experiment_backtracks (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    experiment_id BIGINT NOT NULL,
    from_version VARCHAR(64) NOT NULL,
    to_version VARCHAR(64) NOT NULL,
    reason TEXT NOT NULL,
    actor_id BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_backtrack_exp FOREIGN KEY (experiment_id) REFERENCES experiments(id),
    CONSTRAINT fk_backtrack_actor FOREIGN KEY (actor_id) REFERENCES users(id)
);

INSERT INTO dish_categories (name, created_at)
VALUES ('Bowls', NOW()), ('Drinks', NOW()), ('Snacks', NOW())
ON DUPLICATE KEY UPDATE name = VALUES(name);

INSERT INTO dishes (category_id, name, description, base_price_cents, photo_path, is_published, is_sold_out, created_by, created_at, updated_at)
SELECT c.id, 'Chicken Teriyaki Bowl', 'Protein bowl with rice and vegetables', 1290, '/var/lib/rocket-api/dishes/chicken.jpg', TRUE, FALSE, u.id, NOW(), NOW()
FROM dish_categories c, users u
WHERE c.name = 'Bowls' AND u.username = 'admin'
ON DUPLICATE KEY UPDATE base_price_cents = VALUES(base_price_cents), is_published = TRUE;

INSERT INTO ranking_rules (rule_key, weight, enabled, updated_by, updated_at)
SELECT 'ctr_weight', 0.6, TRUE, u.id, NOW() FROM users u WHERE u.username = 'admin'
ON DUPLICATE KEY UPDATE weight = VALUES(weight), enabled = VALUES(enabled), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at);

INSERT INTO ranking_rules (rule_key, weight, enabled, updated_by, updated_at)
SELECT 'conversion_weight', 0.4, TRUE, u.id, NOW() FROM users u WHERE u.username = 'admin'
ON DUPLICATE KEY UPDATE weight = VALUES(weight), enabled = VALUES(enabled), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at);

INSERT INTO experiment_variants (experiment_id, variant_key, allocation_weight, feature_version, active)
SELECT e.id, 'control', 0.5, 'v1', TRUE FROM experiments e WHERE e.experiment_key = 'intranet_ui_experiment'
ON DUPLICATE KEY UPDATE allocation_weight = VALUES(allocation_weight), active = VALUES(active);

INSERT INTO experiment_variants (experiment_id, variant_key, allocation_weight, feature_version, active)
SELECT e.id, 'variant-a', 0.5, 'v2', TRUE FROM experiments e WHERE e.experiment_key = 'intranet_ui_experiment'
ON DUPLICATE KEY UPDATE allocation_weight = VALUES(allocation_weight), active = VALUES(active);
