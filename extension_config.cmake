# This file is included by DuckDB's build system. It specifies which extension to load

# Extension from this repo
duckdb_extension_load(athena
    SOURCE_DIR ${CMAKE_CURRENT_LIST_DIR}
    LOAD_TESTS
    LINKED_LIBS "${CMAKE_CURRENT_BINARY_DIR}/libduckdb_athena.a"
)

# Any extra extensions that should be built
# e.g.: duckdb_extension_load(json)
