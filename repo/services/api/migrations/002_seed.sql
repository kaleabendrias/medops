INSERT INTO roles (name, description)
VALUES
  ('admin', 'Full administrative access'),
  ('doctor', 'Clinical access to patient workflows'),
  ('nurse', 'Operational care and triage access'),
  ('auditor', 'Read-only audit and compliance visibility')
ON DUPLICATE KEY UPDATE description = VALUES(description);

INSERT INTO hospitals (code, name, city, country, status)
VALUES
  ('HSP-NYC-001', 'Manhattan General Hospital', 'New York', 'United States', 'active'),
  ('HSP-CHI-002', 'Lakeshore Medical Center', 'Chicago', 'United States', 'active'),
  ('HSP-HOU-003', 'Texas Medical Institute', 'Houston', 'United States', 'active')
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  city = VALUES(city),
  country = VALUES(country),
  status = VALUES(status);
