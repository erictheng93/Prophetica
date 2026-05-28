-- SPEC-024 P3.1 codex review P1 #3 follow-up:
-- redemption_orders.idempotency_key was globally UNIQUE in
-- 202605270010_create_redemption_schema.sql. That meant user B reusing user
-- A's idempotency_key would either collide on insert (best case) or — in the
-- in-memory store equivalent — silently return user A's order. Both behaviours
-- leak the wrong order across users.
--
-- Fix: scope idempotency by (user_id, idempotency_key) at the schema layer.
-- The same client-supplied key may now appear under two different users
-- without collision; only same-(user, key) replays converge on the prior
-- order. Mirrors the InMemoryRedemptionStore composite-key change in
-- services/redemption-service/src/store.rs (`idempotency_storage_key`).

ALTER TABLE redemption_orders
    DROP CONSTRAINT redemption_orders_idempotency_key_key;

ALTER TABLE redemption_orders
    ADD CONSTRAINT redemption_orders_user_idempotency_key_unique
    UNIQUE (user_id, idempotency_key);

CREATE INDEX idx_orders_idempotency_lookup
    ON redemption_orders(user_id, idempotency_key);
