use std::{ffi::CString, slice};

use duckdb_athena_rust::duckdb_vector_size;
use duckdb_athena_rust::{DataChunk, Inserter, LogicalTypeId};

use crate::error::{Error, Result};

// Maps Athena data types to DuckDB types
// Supported types are listed here: https://docs.aws.amazon.com/athena/latest/ug/data-types.html
pub fn map_type(col_type: String) -> Result<LogicalTypeId> {
    let type_id = match col_type.as_str() {
        "boolean" => LogicalTypeId::Boolean,
        "tinyint" => LogicalTypeId::Tinyint,
        "smallint" => LogicalTypeId::Smallint,
        "int" | "integer" => LogicalTypeId::Integer,
        "bigint" => LogicalTypeId::Bigint,
        "double" => LogicalTypeId::Double,
        "float" => LogicalTypeId::Float,
        "decimal" => LogicalTypeId::Decimal,
        "string" | "varchar" | "char" => LogicalTypeId::Varchar,
        "date" => LogicalTypeId::Date,
        "timestamp" => LogicalTypeId::Timestamp,
        _ => {
            return Err(Error::DuckDB(format!("Unsupported data type: {col_type}")));
        }
    };

    Ok(type_id)
}

pub unsafe fn populate_column(
    value: &str,
    col_type: LogicalTypeId,
    output: &DataChunk,
    row_idx: usize,
    col_idx: usize,
) {
    match col_type {
        LogicalTypeId::Varchar => set_bytes(output, row_idx, col_idx, value.as_bytes()),
        LogicalTypeId::Bigint => {
            let cvalue = value.parse::<i64>();
            assign(output, row_idx, col_idx, cvalue)
        }
        LogicalTypeId::Integer => {
            let cvalue = value.parse::<i32>();
            assign(output, row_idx, col_idx, cvalue)
        }
        _ => {
            println!("Unsupported data type: {:?}", col_type);
        }
    }
}

unsafe fn assign<T: 'static>(output: &DataChunk, row_idx: usize, col_idx: usize, v: T) {
    get_column_result_vector::<T>(output, col_idx)[row_idx] = v;
}

unsafe fn get_column_result_vector<T>(output: &DataChunk, column_index: usize) -> &'static mut [T] {
    let result_vector = output.flat_vector(column_index);
    // result_vector.as_mut_slice::<T>() or similar _should_ work here
    let ptr = result_vector.as_mut_ptr::<T>();
    slice::from_raw_parts_mut(ptr, duckdb_vector_size() as usize)
}

unsafe fn set_bytes(output: &DataChunk, row_idx: usize, col_idx: usize, bytes: &[u8]) {
    let cs = CString::new(bytes).unwrap();
    let result_vector = &mut output.flat_vector(col_idx);
    result_vector.insert(row_idx, cs.to_str().unwrap());
}
