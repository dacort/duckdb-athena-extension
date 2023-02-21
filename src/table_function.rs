use anyhow::{anyhow, Context, Result};
use futures::executor::block_on;
use std::{
    ffi::{c_void, CString, CStr},
    os::raw::c_char, thread,
};

use aws_sdk_athena::{
    model::{
        QueryExecutionState::{self, *},
        ResultConfiguration, ResultSet,
    },
    output::GetQueryExecutionOutput,
    Client as AthenaClient,
};
use aws_sdk_glue::{
    Client as GlueClient,
};
use duckdb_ext::{ffi::{
    duckdb_bind_info, duckdb_data_chunk, duckdb_free, duckdb_function_info, duckdb_init_info,
}, malloc_struct, Inserter};
use duckdb_ext::table_function::{BindInfo, InitInfo, TableFunction};
use duckdb_ext::{DataChunk, FunctionInfo, LogicalType, LogicalTypeId};

use tokio::{
    runtime::Runtime,
    time::{ Duration},
};

use crate::types::{map_type, populate_column};

#[repr(C)]
struct MyBindDataStruct {
    filename: *mut c_char,
}

impl ScanBindData {
    fn new(uri: &str) -> Self {
        Self {
            filename: CString::new(uri).expect("Bind uri").into_raw(),
        }
    }
}

/// Drop the ScanBindData from C.
///
/// # Safety
unsafe extern "C" fn drop_scan_bind_data_c(v: *mut c_void) {
    let actual = v.cast::<ScanBindData>();
    drop(CString::from_raw((*actual).filename.cast()));
    duckdb_free(v);
}

#[repr(C)]
struct ScanInitData {
    done: bool,
}

impl ScanInitData {
    fn new() -> Self {
        Self { done: false }
    }
}

#[repr(C)]
struct ScanBindData {
    /// Dataset URI
    filename: *mut c_char,
}

#[repr(C)]
struct MyInitDataStruct {

    done: bool, // TODO: support more than *vector size* rows
}

/// # Safety
///
/// .
#[no_mangle]
unsafe extern "C" fn read_athena(info: duckdb_function_info, output: duckdb_data_chunk) {
    println!("In read_athena");
    let info = FunctionInfo::from(info);
    let output = DataChunk::from(output);

    let bind_data = info.bind_data::<MyBindDataStruct>();
    let mut init_data = info.init_data::<MyInitDataStruct>();

    // To start - let's just do a SELECT * mmk, assuming "filename" is a table name
    let tablename = CStr::from_ptr((*bind_data).filename).to_str().expect("No tablename provided");

    if (*init_data).done {
        output.set_len(0);
        return;
    }

    println!("Creating athena client");
    let config = block_on(aws_config::load_from_env());
    let client = AthenaClient::new(&config);
    let result_config = ResultConfiguration::builder()
        .set_output_location(Some("s3://dacort-east/tmp/athena/rust".to_string()))
        .build();

    // let tablename = "amazon_reviews_parquet";
    let query = format!("SELECT * FROM {} LIMIT 10", tablename);

    println!("Building athena query");
    let athena_query = client
        .start_query_execution()
        .set_query_string(Some(query))
        .set_result_configuration(Some(result_config))
        .set_work_group(Some("primary".to_string()))
        .send();

    println!("Running athena query");
    // TODO: Use unwrap_or maybe? Docs recommend not to use this because it can panic.
    let resp = crate::RUNTIME.block_on(athena_query).expect("could not start query");

    let query_execution_id = resp.query_execution_id().unwrap_or_default();
    println!("Query execution id: {}", &query_execution_id);

    let mut state: QueryExecutionState;

    loop {
        let get_query = client
            .get_query_execution()
            .set_query_execution_id(Some(query_execution_id.to_string()))
            .send();
        // .await.expect("could get query status").clone();

        let resp = crate::RUNTIME.block_on(get_query).expect("Could not get query status");
        state = status(&resp).expect("could not get query status").clone();

        match state {
            Queued | Running => {
                // block_on(sleep(Duration::from_secs(5)));
                thread::sleep(Duration::from_secs(5));
                println!("State: {:?}, sleep 5 secs ...", state);
            }
            Cancelled | Failed => {
                println!("State: {:?}", state);

                match crate::RUNTIME.block_on(get_query_result(&client, query_execution_id.to_string())) {
                    Ok(result) => println!("Result: {:?}", result),
                    Err(e) => println!("Result error: {:?}", e),
                }

                break;
            }
            _ => {
                let millis = total_execution_time(&resp).unwrap();
                println!("State: {:?}", state);
                println!("Total execution time: {} millis", millis);

                // When the query results come back, we have the results (in ResultSet.Rows[].Data)
                //and column metadata info (in ResultSetMetadata.ColumnInfo[]).
                // Each Datum has a VarCharValue that we have to cast to our desired data type.

                match crate::RUNTIME.block_on(get_query_result(&client, query_execution_id.to_string())) {
                    Ok(result) => {
                        // println!("Result: {:?}", result);
                        // let rows = result.rows().unwrap();
                        // for i in 0..rows.len() {
                        //     println!("Row: {:?}", rows[i]);
                        // }
                        result_set_to_duckdb_data_chunk(result, &output).expect("Couldn't write results");
                        // output.set_len(rows.len());
                        (*init_data).done = true;
                    }
                    Err(e) => println!("Result error: {:?}", e),
                }
                break;
            }
        }
    }

    // (*init_data).done = true;
    // output.set_len(0);
    println!("Out read_athena");

    // let filename = CStr::from_ptr((*bind_data).filename);

    // let table_result = RUNTIME.block_on(open_table(filename.to_str().unwrap()));

    // if let Err(err) = table_result {
    //     info.set_error(&err.to_string());
    //     return;
    // }

    // let table = table_result.unwrap();

    // let root_dir = Path::new(filename.to_str().unwrap());
    // let mut row_idx: usize = 0;
    // for pq_filename in table.get_files_iter() {
    //     if (*init_data).done {
    //         break;
    //     }
    //     let reader =
    //         SerializedFileReader::new(File::open(root_dir.join(pq_filename)).unwrap()).unwrap();

    //     for row in reader {
    //         for (col_idx, (_key, value)) in row.get_column_iter().enumerate() {
    //             populate_column(value, &output, row_idx, col_idx);
    //         }
    //         row_idx += 1;

    //         assert!(
    //             row_idx < duckdb_vector_size().try_into().unwrap(),
    //             "overflowed vector: {}",
    //             row_idx
    //         );
    //     }
    // }
    // (*init_data).done = true;
    // output.set_size(row_idx as u64);
}

pub fn result_set_to_duckdb_data_chunk(rs: ResultSet, chunk: &DataChunk) -> Result<()> {
    // Fill the row
    // This is asserting the wrong thing (row length vs. column length)
    // assert_eq!(rs.rows().unwrap().len(), chunk.num_columns());
    let rows = rs.rows().unwrap();

    // 1-indexed - the first row is the header :eek:
    for row_idx in 1..rows.len() {
        let row = &rows[row_idx];
        let row_data = row.data().unwrap();
        for col_idx in 0..row_data.len() {
            let value = row_data[col_idx].var_char_value().unwrap();
            let colinfo = &rs.result_set_metadata().unwrap().column_info().unwrap()[col_idx];
            let ddb_type = map_type(colinfo.r#type().unwrap().to_string()).unwrap();
            unsafe { populate_column(value, ddb_type, chunk, row_idx, col_idx) };
        }
    }
    
    chunk.set_len(rows.len());

    Ok(())
}

fn status(resp: &GetQueryExecutionOutput) -> Option<&QueryExecutionState> {
    resp.query_execution().unwrap().status().unwrap().state()
}

fn total_execution_time(resp: &GetQueryExecutionOutput) -> Option<i64> {
    resp.query_execution()
        .unwrap()
        .statistics()
        .unwrap()
        .total_execution_time_in_millis()
}

async fn get_query_result(client: &AthenaClient, query_execution_id: String) -> Result<ResultSet> {
    let resp = client
        .get_query_results()
        .set_query_execution_id(Some(query_execution_id.clone()))
        .send()
        .await
        .with_context(|| {
            format!(
                "could not get query results for query id {}",
                query_execution_id
            )
        })?;

    Ok(resp
        .result_set()
        .ok_or_else(|| anyhow!("could not get query result"))?
        .clone())
}

unsafe extern "C" fn drop_my_bind_data_struct(v: *mut c_void) {
    let actual = v.cast::<MyBindDataStruct>();
    drop(CString::from_raw((*actual).filename.cast()));
    duckdb_free(v);
}

/// # Safety
///
/// .
#[no_mangle]
unsafe extern "C" fn read_athena_bind(bind_info: duckdb_bind_info) {
    println!("In read_athena_bind");
    let bind_info = BindInfo::from(bind_info);
    assert_eq!(bind_info.num_parameters(), 1);

    let param = bind_info.parameter(0);
    let ptr = param.to_string();

    // For now, we're only doing 1 column
    // let typ = LogicalType::new(map_type());
    // bind_info.add_result_column("review_id", typ);

    // Table name is the first param that's getting passed in
    // We need to go to the Glue Data Catalog and fetch the column tables for that table.
    // For now, we only support the `default` table.
    println!("Creating glue client");
    let config = block_on(aws_config::load_from_env());
    let client = GlueClient::new(&config);

    let table = client.get_table().database_name("default").name(param.to_string()).send();
    // let resp = crate::RUNTIME.block_on(table);

    println!("Query table");
    match crate::RUNTIME.block_on(table) {
        Ok(resp) => {
            let columns = resp.table().unwrap().storage_descriptor().unwrap().columns();
            for column in columns.unwrap() {
                let typ = LogicalType::new(map_type(column.r#type().unwrap_or("varchar").to_string()).expect("Could not get type"));
                bind_info.add_result_column(column.name().unwrap(), typ);
            }
        }
        Err(err) => {
            bind_info.set_error(duckdb_ext::Error::DuckDB(err.into_service_error().to_string()));
            return;
        }
    }


    // if let Err(err) = resp {
    //     bind_info.set_error(duckdb_ext::Error::DuckDB(err.to_string()));
    //     return;
    // }
    // let handle = RUNTIME.block_on(open_table(cstring));
    // if let Err(err) = handle {
    //     bind_info.set_error(&err.to_string());
    //     return;
    // }

    // let table = handle.unwrap();
    // let schema = table.schema().expect("no schema");
    // for field in schema.get_fields() {
    //     let typ = LogicalType::new(map_type(field.get_type()));
    //     bind_info.add_result_column(field.get_name(), typ);
    // }

    // let my_bind_data = malloc_struct::<MyBindDataStruct>();
    // (*my_bind_data).filename = CString::new(cstring).expect("c string").into_raw();
    // let bind_data = Box::new(ScanBindData::new(&ptr));
    // bind_info.set_bind_data(Box::into_raw(bind_data).cast(), Some(drop_scan_bind_data_c));

    unsafe {
        let bind_data = malloc_struct::<ScanBindData>();
        (*bind_data).filename = CString::new(ptr).expect("Bind uri").into_raw();
    
        bind_info.set_bind_data(bind_data.cast(), Some(drop_scan_bind_data_c));
    }
    println!("Out read_athena_bind");

    // bind_info.set_bind_data(my_bind_data.cast(), Some(drop_my_bind_data_struct));
}

/// # Safety
///
/// .
#[no_mangle]
unsafe extern "C" fn read_athena_init(info: duckdb_init_info) {
    println!("In read_athena_init");
    let info = InitInfo::from(info);
    // let bind_data = info.bind_data::<ScanBindData>();

    let init_data = Box::new(ScanInitData::new());
    info.set_init_data(Box::into_raw(init_data).cast(), Some(duckdb_free));

    // let mut my_init_data = malloc_struct::<MyInitDataStruct>();
    // (*my_init_data).done = false;
    // info.set_init_data(my_init_data.cast(), Some(duckdb_free));
    println!("Out read_athena_init");
}

pub fn build_table_function_def() -> TableFunction {
    let table_function = TableFunction::new("athena_scan");
    let logical_type = LogicalType::new(LogicalTypeId::Varchar);
    table_function.add_parameter(&logical_type);

    table_function.set_function(Some(read_athena));
    table_function.set_init(Some(read_athena_init));
    table_function.set_bind(Some(read_athena_bind));
    table_function
}

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
}
