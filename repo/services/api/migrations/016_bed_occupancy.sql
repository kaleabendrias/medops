-- Bed occupancy tracking: ties patient identity to bed lifecycle events.
-- check-in creates an occupancy, check-out/transfer closes it.

CREATE TABLE IF NOT EXISTS bed_occupancies (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    bed_id BIGINT NOT NULL,
    patient_id BIGINT NOT NULL,
    checked_in_at DATETIME NOT NULL,
    checked_out_at DATETIME NULL,
    checked_out_reason VARCHAR(64) NULL,
    CONSTRAINT fk_occ_bed FOREIGN KEY (bed_id) REFERENCES beds(id),
    CONSTRAINT fk_occ_patient FOREIGN KEY (patient_id) REFERENCES patients(id)
);

CREATE INDEX idx_occ_bed_active ON bed_occupancies(bed_id, checked_out_at);
CREATE INDEX idx_occ_patient ON bed_occupancies(patient_id);

-- Add optional patient_id to bed_events for traceability
ALTER TABLE bed_events
    ADD COLUMN patient_id BIGINT NULL AFTER to_state,
    ADD CONSTRAINT fk_bed_event_patient FOREIGN KEY (patient_id) REFERENCES patients(id);
