-- Governance Tiered Storage — three distinct physical tables.
--
-- Earlier revisions of this migration kept governance records in a single
-- `governance_records` table with a `tier` discriminator and surfaced each
-- tier through a CREATE OR REPLACE VIEW. That arrangement leaked tier mixing
-- at the storage layer (raw, cleaned, and analytics rows shared the same
-- physical pages, the same indexes, and the same self-referential FK).
-- It also made it impossible for the database to enforce that a "cleaned"
-- record's lineage_source actually points at a raw record (and analytics at
-- cleaned) — the FK was self-referential and the tier column was unchecked.
--
-- This migration replaces that workaround with three distinct physical
-- tables and EXPLICIT cross-table foreign keys, so lineage integrity is
-- enforced by the database itself:
--
--   governance_raw       — bottom tier; no source.
--   governance_cleaned   — FK lineage_source_id -> governance_raw(id).
--   governance_analytics — FK lineage_source_id -> governance_cleaned(id).
--
-- A shared monotonic id sequence (governance_id_sequence) is used so that
-- record ids remain globally unique across the three tiers, which keeps the
-- public API surface (`/governance/records/{id}`) and the audit log entity
-- ids stable and unambiguous.

-- The legacy single-table workaround and its CREATE OR REPLACE VIEW shims
-- (governance_records + governance_raw/cleaned/analytics views) are
-- explicitly torn down before the new physical tables are created. The
-- DROPs are guarded with IF EXISTS so this migration is also idempotent on
-- databases that were never bootstrapped with the legacy schema.
DROP VIEW IF EXISTS governance_analytics;
DROP VIEW IF EXISTS governance_cleaned;
DROP VIEW IF EXISTS governance_raw;
DROP TABLE IF EXISTS governance_records;

CREATE TABLE IF NOT EXISTS governance_id_sequence (
    id BIGINT PRIMARY KEY AUTO_INCREMENT
);

CREATE TABLE IF NOT EXISTS governance_raw (
    id BIGINT PRIMARY KEY,
    lineage_metadata TEXT NOT NULL,
    payload_json LONGTEXT NOT NULL,
    tombstoned BOOLEAN NOT NULL DEFAULT FALSE,
    tombstone_reason TEXT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_gov_raw_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS governance_cleaned (
    id BIGINT PRIMARY KEY,
    lineage_source_id BIGINT NOT NULL,
    lineage_metadata TEXT NOT NULL,
    payload_json LONGTEXT NOT NULL,
    tombstoned BOOLEAN NOT NULL DEFAULT FALSE,
    tombstone_reason TEXT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_gov_cleaned_source
        FOREIGN KEY (lineage_source_id) REFERENCES governance_raw(id),
    CONSTRAINT fk_gov_cleaned_user FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS governance_analytics (
    id BIGINT PRIMARY KEY,
    lineage_source_id BIGINT NOT NULL,
    lineage_metadata TEXT NOT NULL,
    payload_json LONGTEXT NOT NULL,
    tombstoned BOOLEAN NOT NULL DEFAULT FALSE,
    tombstone_reason TEXT NULL,
    created_by BIGINT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT fk_gov_analytics_source
        FOREIGN KEY (lineage_source_id) REFERENCES governance_cleaned(id),
    CONSTRAINT fk_gov_analytics_user FOREIGN KEY (created_by) REFERENCES users(id)
);
