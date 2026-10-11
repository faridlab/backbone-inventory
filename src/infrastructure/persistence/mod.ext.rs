// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The GL reconcile's SQL — uniform across all three posting voucher tables, so it belongs to no
// single entity repository. Declared `user_owned` in metaphor.codegen.yaml.
mod gl_voucher_repository;
mod valuation_overlay_repository;
// Picking-as-projection SQL (transfer header mint, projection probes, location resolution,
// the quant-surface backfill, adjustment-move detection). Declared `user_owned`.
mod stock_picking_repository;
// Quant-driven adjustment SQL (count staging on the quant, staging consume, pending-count
// worklist). Declared `user_owned`.
mod stock_adjustment_repository;
// Picking-batch (SB-1) projection SQL: header mint, membership writes, the stored-compute
// reproject, and the probe reads. Distinct from the generated `picking_batch_repository`
// newtype the read routes use. Declared `user_owned`.
mod picking_batch_projection_repository;
// Scrap-door SQL: header mint, probe, the terminal done-stamp. Distinct from the generated
// `scrap_repository` newtype. Declared `user_owned`.
mod scrap_door_repository;
// Procurement SQL (routes/rules/orderpoints): rule search, orderpoint claims, T11 forecast
// aggregation, move minting, quant housekeep. Declared `user_owned` in metaphor.codegen.yaml.
// `pub mod` (not `mod` + re-export like the others): the minted-move input type is imported
// path-qualified by the service + scheduler to stay distinct from the move engine's own
// `NewMoveRow` re-exported below.
pub mod procurement_repository;
// The hand-written inventory SQL's parameter/projection types. Their repositories are all declared
// `user_owned` in metaphor.codegen.yaml.
pub use gl_voucher_repository::{GlSettlementState, GlVoucher, GlVoucherRepository};
pub use valuation_overlay_repository::{
    ValuationOverlayRepository, PostureRow, LcHeaderRow, LcLineRow, WorksheetRow,
    NewLandedCostRow, NewLcLineRow, NewWorksheetRow,
};
pub use bin_repository::{BinAvailabilityRow, BinBalanceRow, BinWarehouseAvailabilityRow};
pub use stock_ledger_entry_repository::NewSleRow;
// The stock-convergence engine's parameter/projection types (repositories declared `user_owned`
// in metaphor.codegen.yaml): quant reservation apex, move lifecycle writes + T1 picking
// projection, move-line reservation mirror.
pub use quant_repository::{QuantDims, QuantOnHandRow, QuantRow};
pub use stock_move_repository::{LocationFacts, MoveRow, NewMoveRow};
pub use stock_move_line_repository::{MoveLineRow, NewMoveLineRow};
pub use warehouse_repository::NewWarehouseRow;
pub use stock_item_repository::NewStockItemRow;
pub use purchase_receipt_repository::{NewReceiptRow, ReceiptCancelHeaderRow, ReceiptRepostHeaderRow, ReceiptSubmitHeaderRow};
pub use purchase_receipt_item_repository::{NewReceiptItemRow, ReceiptItemRow};
pub use delivery_note_repository::{DeliveryCancelHeaderRow, DeliveryRepostHeaderRow, DeliverySubmitHeaderRow, NewDeliveryRow};
pub use delivery_note_item_repository::{DeliveryCancelItemRow, DeliveryItemRow, NewDeliveryItemRow};
pub use stock_entry_repository::NewTransferRow;
pub use stock_entry_item_repository::NewStockEntryItemRow;
// Procurement surface projection types (repository declared `user_owned` above). The minted-
// move input stays path-qualified (`procurement_repository::NewMoveRow`) because the move
// engine's repository exports its own `NewMoveRow` under this same module.
pub use procurement_repository::{
    ClaimedMoveRow, ForecastComponents, OrderpointComputes, OrderpointRow,
    ProcurementRepository, RuleRow,
};
pub use stock_reconciliation_repository::{NewReconciliationRow, ReconRepostHeaderRow};
pub use stock_reconciliation_item_repository::NewReconciliationItemRow;
// Picking-as-projection + quant-driven adjustment surfaces (repositories declared
// `user_owned` above).
pub use stock_picking_repository::{
    AdjustmentMoveRow, MoveStateRow, NewPickingRow, OperationTypeFacts, StockPickingRepository,
    TransferHeaderRow,
};
pub use stock_adjustment_repository::{
    QuantCountRow, QuantSelector, StagedCountRow, StockAdjustmentRepository,
};
// Picking-batch (SB-1) surface types (repository declared `user_owned` above).
pub use picking_batch_projection_repository::{
    BatchHeaderRow, BatchMemberRow, NewBatchRow, PickingBatchProjectionRepository,
};
// Scrap-door surface types (repository declared `user_owned` above).
pub use scrap_door_repository::{NewScrapRow, ScrapDoorRepository, ScrapRow};
pub mod stock_reservation_repository;
pub use stock_reservation_repository::{
    HeldAtWarehouse, HeldReservation, ReservationRepoError, StockReservationRepository,
};
