-- Down: drop enum types for inventory module
DROP TYPE IF EXISTS valuation_method CASCADE;
DROP TYPE IF EXISTS warehouse_type CASCADE;
DROP TYPE IF EXISTS voucher_type CASCADE;
DROP TYPE IF EXISTS stock_entry_type CASCADE;
DROP TYPE IF EXISTS gl_posting_state CASCADE;
DROP TYPE IF EXISTS doc_status CASCADE;
