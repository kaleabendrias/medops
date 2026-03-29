ALTER TABLE ingestion_tasks
    ADD COLUMN last_incremental_value VARCHAR(512) NULL;

CREATE TABLE IF NOT EXISTS ingestion_task_records (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    task_id BIGINT NOT NULL,
    run_id BIGINT NOT NULL,
    source_url VARCHAR(1024) NOT NULL,
    record_json LONGTEXT NOT NULL,
    content_hash CHAR(64) NOT NULL,
    incremental_value VARCHAR(512) NULL,
    extracted_at DATETIME NOT NULL,
    UNIQUE KEY uniq_ingestion_task_record_hash (task_id, content_hash),
    KEY idx_ingestion_task_records_run (run_id),
    CONSTRAINT fk_ingestion_record_task FOREIGN KEY (task_id) REFERENCES ingestion_tasks(id),
    CONSTRAINT fk_ingestion_record_run FOREIGN KEY (run_id) REFERENCES ingestion_task_runs(id)
);
