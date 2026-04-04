-- Allow member role to place dining orders without clinical patient assignment.
-- Members participate in the dining/campaign system as self-service users.
-- The order.global_access permission bypasses the patient-assignment check in
-- place_order, enabling members to order for any patient (typically themselves
-- in a member self-service context).

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'order.global_access'
FROM roles r
WHERE r.name = 'member'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);
