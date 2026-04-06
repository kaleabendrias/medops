-- Revoke order.global_access from member role to enforce patient-scoped isolation.
-- Members must not be able to read or mutate orders for arbitrary patients.
-- Replace with order.self_service: members can only create and access orders
-- where they are the creator (created_by = user_id).

DELETE rp FROM role_permissions rp
JOIN roles r ON r.id = rp.role_id
WHERE r.name = 'member'
  AND rp.permission_key = 'order.global_access';

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, 'order.self_service'
FROM roles r
WHERE r.name = 'member'
ON DUPLICATE KEY UPDATE permission_key = VALUES(permission_key);
