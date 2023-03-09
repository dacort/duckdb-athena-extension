# DuckDB Athena Extension

> **WARNING** This is a work in progress - things may or may not work as expected 🧙‍♂️

## Limitations

- Only the `default` database is supported
- Not all data types are implemented yet
- 10,000 results are returned by default (use `maxrows=-1` to return everything)
- Pushdown predicates are not supported

## Getting started

The Athena extension is supported in DuckDB v0.7.0 and up. To install the extension, start duckdb with the `unsigned` parameter.

```
> duckdb -unsigned
v0.7.1 b00b93f0b1
D 
```

The first time you use the extension, you need to install it from a custom repository. 

```
SET custom_extension_repository='d2j9pg7mqm9we6.cloudfront.net/athena/latest';
INSTALL athena;
```

Then LOAD the extension. You only need to run the INSTALL command once.

```
LOAD athena;
```

You can now extract data from tables in your default data catalog.

```
select * from athena_scan("noaa_gsod_pds", "s3://results-bucket/prefix");
```

> **Warning** To prevent runaway queries, the extension only returns 10,000 rows by default. If you'd like to return everything, you can add `maxrows=-1` as a parameter inside the function.

```
select * from athena_scan("noaa_gsod_pds", "s3://results-bucket/prefix", maxrows=-1);
```

Filter pushdown is not yet supported so the extension will scan the entire table.

> **Note** The extension uses your environment variables to figure out region and credentials. Make sure to have your access key/secret set.

## Development

- Clone the repo with submodules

```bash
git clone https://github.com/dacort/duckdb-athena-extension.git --recurse-submodules
```

- Build

```bash
cd duckdb-athena-extension
make release
```

- Start up duckdb with the `-unsigned` parameter and your desired AWS_REGION

```bash
AWS_REGION=us-east-1 build/debug/duckdb -unsigned
```

```bash
v0.7.0 f7827396d7
Enter ".help" for usage hints.
D 
```

- Load the extension

```
load 'build/debug/extension/duckdb-athena-extension/athena.duckdb_extension';
```

- Query a single table, also providing where S3 results are written to

```sql
select * from athena_scan('table_name', 's3://<bucket>/athena-results/);
```

> **Warning**: 10,000 results will be returned by default! Use `maxrows=-1` to return the entire table.

```
D select * from athena_scan("amazon_reviews_parquet");
Running Athena query, execution id: 152a20c7-ff32-4a19-bb71-ae0135373ca6
State: Queued, sleep 5 secs ...
Total execution time: 1307 millis
100% ▕████████████████████████████████████████████████████████████▏ 
┌─────────────┬─────────────┬────────────────┬────────────┬────────────────┬───┬─────────┬───────────────────┬──────────────────────┬──────────────────────┬─────────────────┬───────┐
│ marketplace │ customer_id │   review_id    │ product_id │ product_parent │ … │  vine   │ verified_purchase │   review_headline    │     review_body      │   review_date   │ year  │
│   varchar   │   varchar   │    varchar     │  varchar   │    varchar     │   │ varchar │      varchar      │       varchar        │       varchar        │      int64      │ int32 │
├─────────────┼─────────────┼────────────────┼────────────┼────────────────┼───┼─────────┼───────────────────┼──────────────────────┼──────────────────────┼─────────────────┼───────┤
│ US          │ 37441986    │ R2H287L0BUP89U │ B00CT780C2 │ 473048287      │ … │ N       │ Y                 │ Perfect Gift         │ I love giving my s…  │ 140454171422720 │     0 │
│ US          │ 20676035    │ R1222MJHP5QWXE │ B004LLILFA │ 361255549      │ … │ N       │ Y                 │ Five Stars           │ Great gift for out…  │           16170 │  2014 │
│ US          │ 45090731    │ R32ECJRNTB61K8 │ B004LLIL4G │ 307223063      │ … │ N       │ Y                 │ happy birthday card  │ gift cards from Am…  │ 140454171423232 │     0 │
│ US          │ 2207141     │ RLTEU3JZ1IJAA  │ B004LLILDM │ 87389551       │ … │ N       │ Y                 │ Five Stars           │ gracias.             │           16391 │  2014 │
│ US          │ 15258       │ R1ZAX1TN66QOU6 │ B004LLIKVU │ 473048287      │ … │ N       │ Y                 │ easy breezy          │ gift card was sent…  │ 140454171424000 │     0 │
│ ·           │    ·        │       ·        │     ·      │    ·           │ · │ ·       │ ·                 │     ·                │    ·                 │             ·   │    ·  │
├─────────────┴─────────────┴────────────────┴────────────┴────────────────┴───┴─────────┴───────────────────┴──────────────────────┴──────────────────────┴─────────────────┴───────┤
│ 999 rows (40 shown)                                                                                                                                          15 columns (11 shown) │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Credits

- Initial rust DuckDB Extension Framework: https://github.com/Mause/duckdb-extension-framework
- Updated rust extension framework: https://github.com/eto-ai/lance/tree/main/integration/duckdb_lance