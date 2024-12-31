//  Copyright 2023 Lance Authors
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

#include "duckdb_athena_rust.h"

#include <string>

#include "duckdb.hpp"

namespace
{

  auto build_child_list(duckdb_logical_type *member_types, const char **member_names, idx_t member_count)
  {
    duckdb::child_list_t<duckdb::LogicalType> members;
    for (idx_t i = 0; i < member_count; i++)
    {
      members.emplace_back(std::string(member_names[i]), *(duckdb::LogicalType *)member_types[i]);
    }
    return members;
  }

} // namespace

extern "C"
{
  duckdb_logical_type duckdb_create_struct_type(duckdb_logical_type *member_types,
                                                const char **member_names,
                                                idx_t member_count)
  {
    auto *stype = new duckdb::LogicalType;
    *stype = duckdb::LogicalType::STRUCT(build_child_list(member_types, member_names, member_count));
    return reinterpret_cast<duckdb_logical_type>(stype);
    ;
  }
}
