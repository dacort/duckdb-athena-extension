#!/bin/bash

# Usage: ./extension-upload.sh <name> <extension_version> <duckdb_version> <architecture> <s3_bucket> <copy_to_latest>
# <name>                : Name of the extension
# <extension_version>   : Version (commit / version tag) of the extension
# <duckdb_version>      : Version (commit / version tag) of DuckDB
# <architecture>        : Architecture target of the extension binary
# <s3_bucket>           : S3 bucket to upload to
# <copy_to_latest>      : Set this as the latest version ("true" / "false", default: "false")

set -e

ext="build/release/extension/duckdb-athena-extension/$1.duckdb_extension"

# compress extension binary
gzip < $ext > "$1.duckdb_extension.gz"

# upload compressed extension binary to S3
echo "Uploading extension binary to s3://$5/artifacts/$1/$2/$3/$4/$1.duckdb_extension.gz"
aws s3 cp $1.duckdb_extension.gz s3://$5/artifacts/$1/$2/$3/$4/$1.duckdb_extension.gz

if [ $6 = 'true' ]
then
  echo "Also copying to latest"
  aws s3 cp $1.duckdb_extension.gz s3://$5/artifacts/$1/latest/$3/$4/$1.duckdb_extension.gz
fi
# also uplo