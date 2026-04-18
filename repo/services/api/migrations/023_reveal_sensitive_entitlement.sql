-- Replace hardcoded role-name checks in the frontend layer with a database-
-- driven capability key. Previously `can_reveal_revision_fields` branched on
-- `matches!(role, "admin" | "doctor" | "nurse")` in the frontend; this leaks
-- the access policy into client-side code that should not make authorization
-- decisions unilaterally.
--
-- The capability key `reveal_sensitive` is added to `menu_entitlements` for
-- roles whose users are permitted to see unredacted PHI in revision timelines
-- (allergies, contraindications, medical history). The frontend reads this
-- entitlement from the `/rbac/menu-entitlements` response and gates reveal
-- accordingly.
--
-- Roles granted reveal_sensitive: admin, doctor, nurse, auditor.
-- Roles NOT granted it: member, employee, cafeteria.

INSERT INTO menu_entitlements (role_id, menu_key, allowed)
SELECT r.id, 'reveal_sensitive', TRUE
FROM roles r
WHERE r.name IN ('admin', 'doctor', 'nurse', 'auditor')
ON DUPLICATE KEY UPDATE allowed = VALUES(allowed);
