-- The store-level reservation ledger: checkout intents held against a
-- warehouse's availability without touching the quant counters. The quant
-- triangle's reserved_quantity is a MIRROR of open stock-move lines and is
-- recomputed from them, so a cart hold written there would be wiped by the
-- next mirror pass; holds live in their own ledger and the availability
-- read subtracts them at the warehouse grain instead.
CREATE TABLE inventory.stock_reservations (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ref_type      TEXT NOT NULL,
    ref_id        UUID NOT NULL,
    warehouse_id  UUID NOT NULL,
    item_id       UUID NOT NULL,
    qty           NUMERIC(18,4) NOT NULL CHECK (qty > 0),
    status        TEXT NOT NULL DEFAULT 'held'
                  CHECK (status IN ('held', 'released', 'expired', 'consumed')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL,
    settled_at    TIMESTAMPTZ,
    metadata      JSONB NOT NULL DEFAULT '{"created_at": null, "created_by": null, "deleted_at": null, "deleted_by": null, "updated_at": null, "updated_by": null}'::jsonb,
    org_unit_id   UUID NOT NULL DEFAULT NULLIF(current_setting('app.acting_unit_id'::text, true), ''::text)::uuid
);
-- Tenancy posture (the module's strip law): the org_unit column and its
-- GUC default are module-side; the ROW-LEVEL fence and its fill trigger
-- are the composing service's tenancy decorator's to install -- the
-- module declares no policy.

-- One held line per (ref, item): a re-hold replaces, it never stacks.
CREATE UNIQUE INDEX uq_stock_reservations_held_ref_item
    ON inventory.stock_reservations (ref_type, ref_id, item_id)
    WHERE status = 'held';

-- The availability subtraction's lookup.
CREATE INDEX idx_stock_reservations_held_warehouse_item
    ON inventory.stock_reservations (warehouse_id, item_id)
    WHERE status = 'held';

CREATE INDEX idx_stock_reservations_expires
    ON inventory.stock_reservations (expires_at)
    WHERE status = 'held';
