/*
 * because we link twice (once to the rust library, and once to the duckdb library) we need a bridge to export the rust symbols
 * this is that bridge
 */

#include "extension.h"

const char* athena_version_rust(void);
void athena_init_rust(void* db);

DUCKDB_EXTENSION_API const char* athena_version() {
    return athena_version_rust();
}

DUCKDB_EXTENSION_API void athena_init(void* db) {
    athena_init_rust(db);
}
