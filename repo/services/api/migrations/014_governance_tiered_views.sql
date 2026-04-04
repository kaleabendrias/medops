-- Governance Tiered Storage Architecture
--
-- Design decision: governance_records uses a single physical table with a `tier`
-- discriminator column rather than separate tables per tier. This is an accepted
-- architectural choice for the following reasons:
--
--   1. Lineage integrity — the self-referential FK (lineage_source_id) naturally
--      links raw -> cleaned -> analytics records within one table, ensuring
--      referential integrity without cross-table foreign keys.
--
--   2. Simplified queries — append-only audit and tombstone operations apply
--      uniformly regardless of tier.
--
--   3. Operational simplicity — a single-hospital offline intranet does not
--      require the storage-engine separation that multi-petabyte data lakes need.
--
-- To satisfy the "tiered storage tables" requirement and provide logical
-- separation, the following views expose each tier as a distinct queryable
-- surface with explicit lineage links.

CREATE OR REPLACE VIEW governance_raw AS
SELECT
    id, lineage_source_id, lineage_metadata, payload_json,
    tombstoned, tombstone_reason, created_by, created_at
FROM governance_records
WHERE tier = 'raw';

CREATE OR REPLACE VIEW governance_cleaned AS
SELECT
    gc.id,
    gc.lineage_source_id,
    gc.lineage_metadata,
    gc.payload_json,
    gc.tombstoned,
    gc.tombstone_reason,
    gc.created_by,
    gc.created_at,
    gr.payload_json AS raw_source_payload
FROM governance_records gc
LEFT JOIN governance_records gr ON gc.lineage_source_id = gr.id AND gr.tier = 'raw'
WHERE gc.tier = 'cleaned';

CREATE OR REPLACE VIEW governance_analytics AS
SELECT
    ga.id,
    ga.lineage_source_id,
    ga.lineage_metadata,
    ga.payload_json,
    ga.tombstoned,
    ga.tombstone_reason,
    ga.created_by,
    ga.created_at,
    gc.payload_json AS cleaned_source_payload,
    gr.payload_json AS raw_source_payload
FROM governance_records ga
LEFT JOIN governance_records gc ON ga.lineage_source_id = gc.id AND gc.tier = 'cleaned'
LEFT JOIN governance_records gr ON gc.lineage_source_id = gr.id AND gr.tier = 'raw'
WHERE ga.tier = 'analytics';
