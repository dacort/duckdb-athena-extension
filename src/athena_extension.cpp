#define DUCKDB_EXTENSION_MAIN
#include "athena_extension.hpp"

// Include the declarations of things from Rust.
#include "rust.h"

namespace duckdb
{

    void AthenaExtension::Load(DuckDB &db)
    {
        // Call the Rust function to initialize the extension.
        athena_init_rust(&db);
    }

    std::string AthenaExtension::Name()
    {
        return "athena";
    }

} // namespace duckdb

#ifndef DUCKDB_EXTENSION_MAIN
#error DUCKDB_EXTENSION_MAIN not defined
#endif
