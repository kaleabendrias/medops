SET @has_payload_blob := (
    SELECT COUNT(*)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'patient_attachments'
      AND column_name = 'payload_blob'
);

SET @add_payload_blob_sql := IF(
    @has_payload_blob = 0,
    'ALTER TABLE patient_attachments ADD COLUMN payload_blob LONGBLOB NULL AFTER file_size_bytes',
    'SELECT 1'
);

PREPARE stmt_add_payload_blob FROM @add_payload_blob_sql;
EXECUTE stmt_add_payload_blob;
DEALLOCATE PREPARE stmt_add_payload_blob;
