use anyhow::{anyhow, Result};
use aws_sdk_athena::model::ResultSetMetadata;
use aws_sdk_athena::types::SdkError;
use aws_sdk_athena::{error::GetQueryResultsError, model::Row};
use futures::{executor::block_on, Stream};
use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
    pin::Pin,
    task::{Context, Poll},
    thread,
};

use aws_sdk_athena::{
    model::{
        QueryExecutionState::{self, *},
        ResultConfiguration, ResultSet,
    },
    output::{GetQueryExecutionOutput, GetQueryResultsOutput},
    paginator::GetQueryResultsPaginator,
    Client as AthenaClient,
};
use aws_sdk_glue::Client as GlueClient;
use duckdb_ext::table_function::{BindInfo, InitInfo, TableFunction};
use duckdb_ext::{
    ffi::{
        duckdb_bind_info, duckdb_data_chunk, duckdb_free, duckdb_function_info, duckdb_init_info,
    },
    malloc_struct,
};
use duckdb_ext::{DataChunk, FunctionInfo, LogicalType, LogicalTypeId};

use tokio::{runtime::Runtime, time::Duration};

use crate::types::{map_type, populate_column};

#[repr(C)]
struct ScanBindData {
    /// Athena table name and query result output location
    tablename: *mut c_char,
    output_location: *mut c_char,
    limit: *mut i32,
}

impl ScanBindData {
    fn new(tablename: &str, output_location: &str) -> Self {
        Self {
            tablename: CString::new(tablename).expect("Table name").into_raw(),
            output_location: CString::new(output_location)
                .expect("S3 output location")
                .into_raw(),
        }
    }
}

/// Drop the ScanBindData from C.
///
/// # Safety
unsafe extern "C" fn drop_scan_bind_data_c(v: *mut c_void) {
    let actual = v.cast::<ScanBindData>();
    drop(CString::from_raw((*actual).tablename.cast()));
    drop(CString::from_raw((*actual).output_location.cast()));
    duckdb_free(v);
}

struct ResultStream {
    stream:
        Pin<Box<dyn Stream<Item = Result<GetQueryResultsOutput, SdkError<GetQueryResultsError>>>>>,
}

impl ResultStream {
    pub fn new(
        stream: Box<
            dyn Stream<Item = Result<GetQueryResultsOutput, SdkError<GetQueryResultsError>>>,
        >,
    ) -> Self {
        Self {
            stream: stream.into(),
        }
    }
}

impl Stream for ResultStream {
    type Item = Result<GetQueryResultsOutput, SdkError<GetQueryResultsError>>;

    // https://stackoverflow.com/questions/72926989/how-to-implement-trait-futuresstreamstream was ridiculously helpful
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut().stream.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(v) => {
                // Do what you need to do with v here.
                Poll::Ready(v)
            }
        }
    }
}
#[repr(C)]
struct ScanInitData {
    stream: *mut ResultStream,
    pagination_index: u32,
    done: bool,
}

impl ScanInitData {
    fn new(stream: Box<ResultStream>) -> Self {
        Self {
            stream: Box::into_raw(stream),
            done: false,
            pagination_index: 0,
        }
    }
}

/// # Safety
///
/// .
#[no_mangle]
unsafe extern "C" fn read_athena(info: duckdb_function_info, output: duckdb_data_chunk) {
    let info = FunctionInfo::from(info);
    let mut output = DataChunk::from(output);

    let init_data = info.init_data::<ScanInitData>();
    // if (*init_data).done {
    //     output.set_len(0);
    //     return;
    // }

    let batch = match crate::RUNTIME
        .block_on(async { futures::StreamExt::next(&mut (*(*init_data).stream).stream).await })
    {
        Some(Ok(b)) => Some(b),
        Some(Err(e)) => {
            info.set_error(duckdb_ext::Error::DuckDB(e.to_string()));
            return;
        }
        None => None,
    };

    if let Some(b) = batch {
        let mut rows = b.result_set().unwrap().rows().unwrap();
        // Athena returns the header in the results 0_o but only in the first page
        if (*init_data).pagination_index == 0 {
            rows = &rows[1..];
        }
        let metadata = b.result_set().unwrap().result_set_metadata().unwrap();
        result_set_to_duckdb_data_chunk(rows, metadata, &mut output)
            .expect("Couldn't write results");
    } else {
        (*init_data).done = true;
        output.set_len(0);
    }

    (*init_data).pagination_index += 1;
}

pub fn result_set_to_duckdb_data_chunk(
    rows: &[Row],
    metadata: &ResultSetMetadata,
    chunk: &DataChunk,
) -> Result<()> {
    // Fill the row
    // This is asserting the wrong thing (row length vs. column length)
    // assert_eq!(rs.rows().unwrap().len(), chunk.num_columns());
    // let rows = &rs.rows().unwrap()[1..];
    let result_size = rows.len();

    for row_idx in 0..result_size {
        let row = &rows[row_idx];
        let row_data = row.data().unwrap();
        for col_idx in 0..row_data.len() {
            let value = row_data[col_idx].var_char_value().unwrap();
            let colinfo = &metadata.column_info().unwrap()[col_idx];
            let ddb_type = map_type(colinfo.r#type().unwrap().to_string()).unwrap();
            unsafe { populate_column(value, ddb_type, chunk, row_idx, col_idx) };
        }
    }

    chunk.set_len(result_size);

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
    let resp = anyhow::Context::with_context(
        client
            .get_query_results()
            .set_query_execution_id(Some(query_execution_id.clone()))
            .send()
            .await,
        || {
            format!(
                "could not get query results for query id {}",
                query_execution_id
            )
        },
    )?;

    Ok(resp
        .result_set()
        .ok_or_else(|| anyhow!("could not get query result"))?
        .clone())
}

async fn get_query_result_paginator(
    client: &AthenaClient,
    query_execution_id: String,
) -> GetQueryResultsPaginator {
    client
        .get_query_results()
        .set_query_execution_id(Some(query_execution_id.clone()))
        .into_paginator()
}

/// # Safety
///
/// .
#[no_mangle]
unsafe extern "C" fn read_athena_bind(bind_info: duckdb_bind_info) {
    let bind_info = BindInfo::from(bind_info);
    assert_eq!(bind_info.num_parameters(), 2);

    let tablename = bind_info.parameter(0);
    let output_location = bind_info.parameter(1);

    // Table name is the first param that's getting passed in
    // We need to go to the Glue Data Catalog and fetch the column tables for that table.
    // For now, we only support the `default` table.
    let config = block_on(aws_config::load_from_env());
    let client = GlueClient::new(&config);

    let table = client
        .get_table()
        .database_name("default")
        .name(tablename.to_string())
        .send();

    match crate::RUNTIME.block_on(table) {
        Ok(resp) => {
            let columns = resp
                .table()
                .unwrap()
                .storage_descriptor()
                .unwrap()
                .columns();
            for column in columns.unwrap() {
                let typ = LogicalType::new(
                    map_type(column.r#type().unwrap_or("varchar").to_string())
                        .expect("Could not get type"),
                );
                bind_info.add_result_column(column.name().unwrap(), typ);
            }
        }
        Err(err) => {
            bind_info.set_error(duckdb_ext::Error::DuckDB(
                err.into_service_error().to_string(),
            ));
            return;
        }
    }

    unsafe {
        let bind_data = malloc_struct::<ScanBindData>();
        (*bind_data).tablename = CString::new(tablename.to_string())
            .expect("Table name")
            .into_raw();
        (*bind_data).output_location = CString::new(output_location.to_string())
            .expect("S3 output location")
            .into_raw();

        bind_info.set_bind_data(bind_data.cast(), Some(drop_scan_bind_data_c));
    }
}

/// # Safety
///
/// .
/// Creates the initial Athena query and waits for it to be done.
#[no_mangle]
unsafe extern "C" fn read_athena_init(info: duckdb_init_info) {
    let info = InitInfo::from(info);
    let bind_info = info.bind_data::<ScanBindData>();
    // assert_eq!(bind_info.num_parameters(), 2);

    // Extract the table name and output location from
    let tablename = CStr::from_ptr((*bind_info).tablename).to_str().unwrap();
    let output_location = CStr::from_ptr((*bind_info).output_location)
        .to_str()
        .unwrap();

    let config = block_on(aws_config::load_from_env());
    let client = AthenaClient::new(&config);
    let result_config = ResultConfiguration::builder()
        .set_output_location(Some(output_location.to_owned()))
        .build();

    let query = format!("SELECT * FROM {}", tablename);

    let athena_query = client
        .start_query_execution()
        .set_query_string(Some(query))
        .set_result_configuration(Some(result_config))
        .set_work_group(Some("primary".to_string()))
        .send();

    // TODO: Use unwrap_or maybe? Docs recommend not to use this because it can panic.
    let resp = crate::RUNTIME
        .block_on(athena_query)
        .expect("could not start query");

    let query_execution_id = resp.query_execution_id().unwrap_or_default();
    println!(
        "Running Athena query, execution id: {}",
        &query_execution_id
    );

    let mut state: QueryExecutionState;

    loop {
        let get_query = client
            .get_query_execution()
            .set_query_execution_id(Some(query_execution_id.to_string()))
            .send();

        let resp = crate::RUNTIME
            .block_on(get_query)
            .expect("Could not get query status");
        state = status(&resp).expect("could not get query status").clone();

        match state {
            Queued | Running => {
                thread::sleep(Duration::from_secs(5));
                println!("State: {:?}, sleep 5 secs ...", state);
            }
            Cancelled | Failed => {
                println!("State: {:?}", state);

                match crate::RUNTIME
                    .block_on(get_query_result(&client, query_execution_id.to_string()))
                {
                    Ok(result) => println!("Result: {:?}", result),
                    Err(e) => println!("Result error: {:?}", e),
                }

                break;
            }
            _ => {
                let millis = total_execution_time(&resp).unwrap();
                println!("Total execution time: {} millis", millis);

                // let stream = match crate::RUNTIME.block_on(async {
                //     let paginator = get_query_result_paginator(&client, query_execution_id.to_string()).await;
                //     let results = paginator.send();
                //     results
                // }) {
                //     Ok(s) => Box::new(s),
                // };
                let paginator = crate::RUNTIME.block_on(async {
                    get_query_result_paginator(&client, query_execution_id.to_string()).await
                });
                let stream = paginator.send();

                let init_data = Box::new(ScanInitData::new(Box::new(ResultStream::new(Box::new(
                    stream,
                )))));
                info.set_init_data(Box::into_raw(init_data).cast(), Some(duckdb_free));
                break;
            }
        }
    }
}

pub fn build_table_function_def() -> TableFunction {
    let table_function = TableFunction::new("athena_scan");
    let logical_type = LogicalType::new(LogicalTypeId::Varchar);
    table_function.add_parameter(&logical_type);
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
