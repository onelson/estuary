#!/usr/bin/env bash
RUST_LOG="actix_web=INFO,estuary=INFO" \
target/debug/estuary --base-url=http://localhost:7878 \
--index-dir=/tmp/estuary/index \
--crate-dir=/tmp/estuary/crates