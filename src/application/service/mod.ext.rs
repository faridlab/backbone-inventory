// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// Hand-authored (user-owned): domain events, the GL-posting port, the valuation engine + writes,
// the read-model (availability/stock-balance), and the DeliveryRequested intake.
pub mod inventory_events;
pub mod inventory_gl;
pub mod inventory_write_service;
// The write surface, chunked: each is an `impl InventoryWriteService` block over the vocabulary
// that stays in `inventory_write_service` (so the `inventory_write_service::{NewReceipt, ...}`
// import paths are unchanged).
pub mod inventory_masters;
pub mod inventory_receipt;
pub mod inventory_delivery;
pub mod inventory_transfer;
pub mod inventory_reconciliation;
// The picking-batch satellite (SB-1): header mint, membership verbs, projection probes.
// The batch state is a stored compute over its member pickings — this file never writes it.
pub mod inventory_batch;
// The scrap satellite door: draft header, process through the ONE move engine (scrapped
// move to the loss sink), terminal done-stamp. Deferred-GL form for the HTTP surface.
pub mod inventory_scrap;
pub mod inventory_read;
// The two-scope availability read (display vs checkout) at one warehouse pivot, computed
// fresh per call off the quant reservation triangle — plus the AvailabilityScopePort
// contract (refusing default) a composing service fills.
pub mod availability_scope;
pub mod inventory_intake;
pub mod inventory_cancellation;
// The converged stock-move engine (_action_confirm / _action_assign / _action_done /
// _action_cancel) over quant-grain reservation — the stock-convergence service layer. Also the
// MovePipeline impl the scheduler drives. Declared `user_owned`.
pub mod inventory_move_engine;
// Procurement configuration (stock.route / stock.rule / warehouse.orderpoint): rule
// selection, pull/push move minting, the orderpoint computes, and the MovePipeline port the
// stock-move engine implements for the daily scheduler. Declared `user_owned`.
pub mod procurement_service;
// The org-scoped posting posture (cost-method vocabulary, the perpetual/periodic axis,
// the anglo-saxon delivery-debit swap) the door posts and the move engine consult. The
// `_custom` suffix keeps it outside generator territory by naming convention.
pub mod inventory_posture;
// Landed-cost documents: draft → validate (split by quantity/value/weight over the target
// receipt's DONE moves, remaining-share revaluation through the engine's adjustment verb,
// one door-owned GL envelope) → done; cancel from draft only. Also `_custom`-named.
pub mod landed_cost_service_custom;
pub mod inventory_reservation;
pub use inventory_reservation::{
    HoldAckLine, HoldLine, ReservationError, ReservationService,
};
pub use inventory_events::{
    InventoryEvent, InventoryEventSink, OrderpointTriggered, StockDelivered, StockMoved,
    StockReceived, StockReconciled,
};
pub use inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostLine, GlPostRejected, GlPostSink,
};
pub use inventory_write_service::{
    DeliveryLine, InventoryError, InventoryWriteService, NewDelivery, NewReceipt, NewReconciliation,
    NewStockItem, NewTransfer, NewWarehouse, ReceiptLine, ReconLine, SubmitOutcome,
};
pub use inventory_read::{AvailabilityView, InventoryReadService, QuantAvailability, StockBalance};
pub use availability_scope::{
    AvailabilityScopeError, AvailabilityScopePort, AvailabilityScopeRead, CheckoutDemand,
    CheckoutFreeQty, RefusingAvailabilityScopePort, ScopedAvailability, SoldOutVerdict,
};
pub use inventory_intake::{
    DeliveryIntake, DeliveryRequestLine, DeliveryRequested, ReceiptExpected, ReceiptRequestLine,
};
pub use procurement_service::{
    AssignSweepReport, MovePipeline, MovePipelineError, NewOrderpoint, NewRoute, NewRouteRule,
    ProcurementError, ProcurementRequest, ProcurementService, ReorderOutcome,
};
pub use inventory_move_engine::{
    BackorderPolicy, MoveAssignOutcome, MoveDoneOutcome, MoveGlDirective, NewStockMove,
};
