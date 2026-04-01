DROP INDEX uniq_order_idempotency_key ON dining_orders;

CREATE UNIQUE INDEX uniq_order_idempotency_user_key
ON dining_orders (idempotency_key, created_by);
