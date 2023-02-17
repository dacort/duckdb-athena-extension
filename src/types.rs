use duckdb_extension_framework::constants::LogicalTypeId;

/// Maps Deltalake types to DuckDB types
pub fn map_type() -> LogicalTypeId {
    LogicalTypeId::Varchar
}