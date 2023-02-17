/*
 * because we link twice (once to the rust library, and once to the duckdb library) we need a bridge to export the rust symbols
 * this is that bridge
 */

#include "wrapper.h"

const char* athenatable_version_rust(void);
void athenatable_init_rust(void* db);

DUCKDB_EXTENSION_API const char* athenatable_version() {
    return athenatable_version_rust();
}

DUCKDB_EXTENSION_API void athenatable_init(void* db) {
    athenatable_init_rust(db);
}
