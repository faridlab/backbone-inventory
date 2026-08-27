# Stock satellite disposition

Status of the `stock` satellite family (the SB rows of the Odoo misc-features spec) in
this module. Every satellite the product wave will not consume is **shed explicitly** —
this file is the fence/ride record, never a silent drop.

## Landed

| Satellite | What landed | Where |
|---|---|---|
| `stock.picking.batch` (SB-1) | Batch entity grouping pickings; `state` is a STORED COMPUTE over member picking states (least-advanced live member; cancelled members drop out; auto-cancel when emptied after having grouped work). Membership writes + the engine's picking-projection cascade are the only recompute drivers — never a hand-set batch state. | `schema/models/batch.model.yaml`, `src/infrastructure/persistence/picking_batch_projection_repository.rs`, `src/application/service/inventory_batch.rs`, migration `20260827140000` |
| `stock.scrap` | Scrap door: draft header → process mints ONE `scrapped` move (source stock location → the inventory-loss sink) and drives it confirm → assign → done through the stock-move engine; the header closes `done` bound to that move. No second estate. HTTP surface is the deferred-GL form. | `schema/models/scrap.model.yaml`, `src/infrastructure/persistence/scrap_door_repository.rs`, `src/application/service/inventory_scrap.rs` |
| `stock.scrap.reason.tag` | Reason-tag master data with the R7 name uniques (shared and per-company partial indexes). | `schema/models/scrap.model.yaml` |
| `stock.package.type` / `stock.storage.category` / `stock.storage.category.capacity` / `stock.putaway.rule` | The package/storage/putaway vocabulary: R10 barcode unique, R8/R9 capacity uniques, R15/16/17 positivity CHECKs, the capacity target XOR, and the T12 putaway `storage_category_id` stored derivation (trigger — the column is never free). | `schema/models/storage.model.yaml`, migration `20260827140100` |

## Shed — fence/ride rows

### `stock.dropshipping` (route-level dropship bypass)

**What it is.** A procurement-route shape where a customer demand is satisfied by a
supplier shipping directly to the customer: the picking legs collapse to a single
dropship move pair (supplier → customer) with no warehouse touch, and the PO's receipt
validates the dropship move instead of a stock receipt.

**Why not now.** Dropshipping is a *buying* concern expressed through inventory's route
engine: it needs `purchase_stock`-shape rule actions (`buy` + dropship picking types on
the supplier location) and a PO↔move linkage (`purchase_line_id` on moves, the
`_action_confirm` PO hook). Buying is consumed only as read-surface in the current wave;
its order-confirm verbs do not drive inventory yet. Minting the dropship route vocabulary
before that driver exists would leave dead master data the engine never selects.

**What riding it would need.** (1) A `dropship` picking-type pair + the supplier↔customer
locations on the route tree (reference seed rows); (2) a rule action extension
(`dropship` / `buy`) in `procurement_repository::search_rule` that skips the warehouse
legs and mints supplier→customer directly; (3) the buying-side confirm hook calling the
pull; (4) a probe test that no warehouse quant ever moves for a dropship-fulfilled line.

### `stock.sms` (SMS/notify overlay on picking events)

**What it is.** Odoo's `stock_sms` posts an SMS to the customer when a delivery picking
validates — a mail-stack integration (`mail.thread`, SMS gateway credentials, templates).

**Why not now.** Pure presentation-layer overlay with zero stock semantics. The module
deliberately owns no mail/SMS stack; notification routing is the composing service's
concern. The event the overlay keys on already exists — the engine publishes
`InventoryEvent::MoveDone` / the delivery submit — so an SMS consumer can ride the event
sink without any inventory change.

**What riding it would need.** A consumer subscribed to the delivery `MoveDone` event in
the composing service (or the notification module), plus its own gateway config. Nothing
in this module.

### `stock_maintenance` / `stock_fleet` (equipment/vehicle bridges)

**What it is.** Odoo bridges that attach equipment (maintenance) or vehicles (fleet) to
pickings/moves — tracking which forklift did a move, which truck carried a delivery.

**Why not now.** There is no equipment/vehicle master anywhere in the consumed module
set; the bridges would be nullable FK columns pointing at nothing. The tracking grain the
wave actually needs (which operator/user grouped and validated a batch) already rides the
batch header's `user_id` and the move's audit metadata.

**What riding it would need.** (1) An equipment/vehicle master module with its own
fences; (2) nullable `equipment_id`/`vehicle_id` on move lines (logical FKs, cross-module
convention); (3) an assignment verb on the picking validate surface; (4) probe tests that
the columns stay optional so the existing flows never require them.

## Deliberately deferred (in-scope adjacent, not shed)

- **Auto-batching / wave wizard.** Odoo's batch-creation wizard (`action_add_to_batch`,
  filter-driven auto-grouping) is operator UX over the same membership verbs that landed;
  the service verbs (`create_batch` / `add_picking_to_batch`) are the complete write
  surface. A wizard is a UI concern for the webapp, not a module verb.
- **`stock.package` history.** Odoo's `package_history` tracks a package's move-line
  trail as its own table. The move-line reservation mirror already records every package
  placement (`result_package_id` per line, `date_done` on the move); a history view over
  those rows is a read-model, not a second writer. Riding it later = a SQL view, no new
  table.
- **Full putaway suggestion engine.** T12 landed the rule vocabulary + stored derivation;
  the *suggester* (pick the rule matching item/category/package and return the
  destination bin) is a read-model over `putaway_rules` the picking surface can grow when
  bin-level putaway UX arrives.
