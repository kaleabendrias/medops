INSERT INTO roles (name, description)
VALUES
  ('admin', 'Full administrative access'),
  ('doctor', 'Clinical access to patient workflows'),
  ('nurse', 'Operational care and triage access'),
  ('auditor', 'Read-only audit and compliance visibility')
ON DUPLICATE KEY UPDATE description = VALUES(description);

INSERT INTO hospitals (code, name, city, country, status)
VALUES
  ('HSP-AMS-001', 'Amsterdam Central Hospital', 'Amsterdam', 'Netherlands', 'active'),
  ('HSP-BER-002', 'Berlin Care Institute', 'Berlin', 'Germany', 'active'),
  ('HSP-LON-003', 'London West Medical Center', 'London', 'United Kingdom', 'active')
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  city = VALUES(city),
  country = VALUES(country),
  status = VALUES(status);
