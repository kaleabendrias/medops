-- Enforce domain constraints for Ticket Split workflow
ALTER TABLE order_tickets
    ADD CONSTRAINT chk_split_by CHECK (split_by IN ('pickup_point', 'kitchen_station')),
    ADD CONSTRAINT chk_quantity_positive CHECK (quantity > 0);
