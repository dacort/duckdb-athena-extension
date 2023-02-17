use anyhow::{anyhow, Context, Result};
use std::{os::raw::c_char, ffi::{CStr, CString, c_void}};
use futures::executor::block_on;


use aws_sdk_athena::{
    model::{
        QueryExecutionState::{self, *},
        ResultConfiguration, ResultSet,
    },
    output::GetQueryExecutionOutput,
    Client,
};
use duckdb_extension_framework::{
    constants::LogicalTypeId,
    duckly::{duckdb_data_chunk, duckdb_function_info, duckdb_init_info, duckdb_free, duckdb_bind_info},
    DataChunk, FunctionInfo, LogicalType, TableFunction, InitInfo, malloc_struct, BindInfo,
};
use tokio::{runtime::Runtime, time::{sleep, Duration}};

#[repr(C)]
struct MyBindDataStruct {
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
    let info = FunctionInfo::from(info);
    let output = DataChunk::from(output);

    let bind_data = info.get_bind_data::<MyBindDataStruct>();
    let mut init_data = info.get_init_data::<MyInitDataStruct>();

    // To start - let's just do a SELECT * mmk, assuming "filename" is a table name
    let tablename = CStr::from_ptr((*bind_data).filename);

    let config = block_on(aws_config::load_from_env());
    let client = Client::new(&config);
    let result_config = ResultConfiguration::builder()
        .set_output_location(Some("s3://dacort/tmp/athena/rust".to_string()))
        .build();
    
    let query = format!("SELECT * FROM {} LIMIT 10", tablename.to_str().unwrap());
    let athena_query = client
        .start_query_execution()
        .set_query_string(Some(query))
        .set_result_configuration(Some(result_config))
        .set_work_group(Some("primary".to_string()))
        .send();
    
    // TODO: Use unwrap_or maybe? Docs recommend not to use this because it can panic.
    let resp = block_on(athena_query).expect("could not start query");
    
    let query_execution_id = resp.query_execution_id().unwrap_or_default();
    println!("Query execution id: {}", &query_execution_id);

    let mut state: QueryExecutionState;

    loop {
        let get_query = client
            .get_query_execution()
            .set_query_execution_id(Some(query_execution_id.to_string()))
            .send();
            // .await.expect("could get query status").clone();

        let resp = block_on(get_query).expect("Could not get query status");
        state = status(&resp).expect("could not get query status").clone();

        match state {
            Queued | Running => {
                block_on(sleep(Duration::from_secs(5)));
                println!("State: {:?}, sleep 5 secs ...", state);
            }
            Cancelled | Failed => {
                println!("State: {:?}", state);

                match block_on(get_query_result(&client, query_execution_id.to_string())) {
                    Ok(result) => println!("Result: {:?}", result),
                    Err(e) => println!("Result error: {:?}", e),
                }

                break;
            }
            _ => {
                let millis = total_execution_time(&resp).unwrap();
                println!("State: {:?}", state);
                println!("Total execution time: {} millis", millis);

                match block_on(get_query_result(&client, query_execution_id.to_string())) {
                    Ok(result) => {
                        println!("Result: {:?}", result);
                        let mut row_idx: usize = 0;
                        for row in result.rows() {
                            row_idx += 1;
                            println!("Row: {:?}", row);
                        }
                        output.set_size(row_idx as u64);
                    },
                    Err(e) => println!("Result error: {:?}", e),
                }

                break;
            }
        }
    }
    

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

async fn get_query_result(client: &Client, query_execution_id: String) -> Result<ResultSet> {
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
    let bind_info = BindInfo::from(bind_info);
    assert_eq!(bind_info.get_parameter_count(), 1);

    let param = bind_info.get_parameter(0);
    let ptr = param.get_varchar();
    let cstring = ptr.to_str().unwrap();

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

    let my_bind_data = malloc_struct::<MyBindDataStruct>();
    (*my_bind_data).filename = CString::new(cstring).expect("c string").into_raw();

    bind_info.set_bind_data(my_bind_data.cast(), Some(drop_my_bind_data_struct));
}

/// # Safety
///
/// .
#[no_mangle]
unsafe extern "C" fn read_athena_init(info: duckdb_init_info) {
    let info = InitInfo::from(info);

    let mut my_init_data = malloc_struct::<MyInitDataStruct>();
    (*my_init_data).done = false;
    info.set_init_data(my_init_data.cast(), Some(duckdb_free));
}

pub fn build_table_function_def() -> TableFunction {
    let table_function = TableFunction::new();
    table_function.set_name("read_athena");
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