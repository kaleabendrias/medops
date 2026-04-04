-- Encrypt patient revision diff payloads at rest.
-- The existing diff_before / diff_after TEXT columns are retained for backwards
-- compatibility but will store "[ENCRYPTED]" going forward.  The actual
-- ciphertext lives in the new LONGTEXT columns.

ALTER TABLE patient_revisions
    ADD COLUMN diff_before_cipher LONGTEXT NULL AFTER diff_before,
    ADD COLUMN diff_after_cipher  LONGTEXT NULL AFTER diff_after,
    ADD COLUMN encryption_key_version VARCHAR(32) NOT NULL DEFAULT 'v1' AFTER diff_after_cipher;
