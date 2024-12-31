#  Copyright 2023 Lance Authors
#
#  Licensed under the Apache License, Version 2.0 (the "License");
#  you may not use this file except in compliance with the License.
#  You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.

# Still need to use cmake to link to duckdb via `build_loadable_extension` macro.
#

cmake_minimum_required(VERSION 3.5)

if (POLICY CMP0135)
    cmake_policy(SET CMP0135 NEW)
endif ()

set(TARGET_NAME athena)
project(${TARGET_NAME} VERSION 0.3)
set(EXTENSION_NAME ${TARGET_NAME}_extension)
set(LOADABLE_EXTENSION_NAME ${TARGET_NAME}_loadable_extension)

if (APPLE)
  # POLICY CMP0042
  set(CMAKE_MACOSX_RPATH 1)
  # I think this is automatically handled by the duckdb Makefile/CMakefiles
#   SET(CMAKE_OSX_ARCHITECTURES "x86_64;arm64" CACHE STRING "Build architectures for Mac OS X" FORCE)
endif()

option(OSX_BUILD_AARCH64 "Build aarch64/arm64 binary on OSX." FALSE)
if (OSX_BUILD_AARCH64)
  if (NOT APPLE)
    error("This only makes sense on OSX")
  endif()
  set(Rust_CARGO_TARGET "aarch64-apple-darwin")
  SET(CMAKE_OSX_ARCHITECTURES "arm64" CACHE STRING "Build architecture for Mac OS X" FORCE)
else()
  SET(CMAKE_OSX_ARCHITECTURES "x86_64" CACHE STRING "Build architectures for Mac OS X" FORCE)
endif()

include(FetchContent)

FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG v0.5 # Optionally specify a commit hash, version tag or branch here
)
set(BUILD_UNITTESTS FALSE) # Disable unit test build in duckdb

FetchContent_MakeAvailable(Corrosion)

#set(EXTERNAL_EXTENSION_DIRECTORIES ${CMAKE_CURRENT_SOURCE_DIR})

corrosion_import_crate(MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml)
include_directories(${CMAKE_CURRENT_SOURCE_DIR}/duckdb/src/include)
include_directories(src/include)

# Now again for aarch64/arm64
# set(Rust_CARGO_TARGET "aarch64-apple-darwin")
# corrosion_import_crate(MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml)

set(EXTENSION_SOURCES src/athena_extension.cpp src/include/athena_extension.hpp)

SET(EXTENSION_STATIC_BUILD 1)
set(PARAMETERS "-warnings")
build_static_extension(${TARGET_NAME} ${EXTENSION_SOURCES})
build_loadable_extension(${TARGET_NAME} ${PARAMETERS} ${EXTENSION_SOURCES})

set_target_properties(${EXTENSION_NAME} PROPERTIES LINKER_LANGUAGE CXX)
target_link_libraries(${EXTENSION_NAME}
        "${CMAKE_CURRENT_BINARY_DIR}/libduckdb_athena.a"
        duckdb_static
)
target_link_libraries(${LOADABLE_EXTENSION_NAME}
        "${CMAKE_CURRENT_BINARY_DIR}/libduckdb_athena.a"
        duckdb_static
)

if (APPLE)
        target_link_libraries(${EXTENSION_NAME}
                "-framework CoreFoundation"
                "-framework Security")
endif()

install(
  TARGETS ${EXTENSION_NAME}
  EXPORT "${DUCKDB_EXPORT_SET}"
  LIBRARY DESTINATION "${INSTALL_LIB_DIR}"
  ARCHIVE DESTINATION "${INSTALL_LIB_DIR}")
