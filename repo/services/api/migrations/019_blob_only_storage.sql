-- Enforce MySQL as sole system of record for attachment payloads.
-- Backfill any NULL blobs from legacy rows, then make column NOT NULL
-- and drop the filesystem fallback column.

UPDATE patient_attachments
SET payload_blob = X''
WHERE payload_blob IS NULL;

ALTER TABLE patient_attachments
    MODIFY COLUMN payload_blob LONGBLOB NOT NULL;

ALTER TABLE patient_attachments
    DROP COLUMN storage_path;
