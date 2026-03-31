ALTER TABLE patient_visit_notes
    ADD COLUMN note_cipher LONGTEXT NULL AFTER note,
    ADD COLUMN encryption_key_version VARCHAR(32) NOT NULL DEFAULT 'v1' AFTER note_cipher;
