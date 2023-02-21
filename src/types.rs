use std::ffi::CString;

use duckdb_ext::{LogicalTypeId, DataChunk, Inserter};

use crate::error::{Error, Result};

// Maps Athena data types to DuckDB types
// Supported types are listed here: https://docs.aws.amazon.com/athena/latest/ug/data-types.html
pub fn map_type(col_type: String) -> Result<LogicalTypeId> {
    let type_id = match col_type.as_str() {
        "string" | "varchar" => LogicalTypeId::Varchar,
        "tinyint" => LogicalTypeId::Tinyint,
        "int" | "integer" => LogicalTypeId::Integer,
        "bigint" => LogicalTypeId::Bigint,
        _ => {
            return Err(Error::DuckDB(format!("Unsupported data type: {col_type}")));
        }
    };

    Ok(type_id)
}


pub unsafe fn populate_column(value: &str, col_type: LogicalTypeId, output: &DataChunk, row_idx: usize, col_idx: usize) {
    match col_type {
        LogicalTypeId::Varchar => {
            set_bytes(output, row_idx, col_idx, value.as_bytes())
        }
        _ => {
            // println!("Unsupported data type: {:?}", col_type);
        }
    }
}

unsafe fn set_bytes(output: &DataChunk, row_idx: usize, col_idx: usize, bytes: &[u8]) {
    let cs = CString::new(bytes).unwrap();
    let result_vector = &mut output.flat_vector(col_idx);
    result_vector.insert(row_idx, cs.to_str().unwrap());
}
