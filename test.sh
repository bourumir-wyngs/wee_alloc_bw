#!/usr/bin/env bash

set -Eeuo pipefail

cd "$(dirname "$0")"

run_nextest() {
    local dir="$1"
    shift
    local args=("$@")

    (
        cd "$dir"
        if ! time cargo nextest run --hide-progress-bar "${args[@]}"; then
            printf 'cargo nextest run failed in %s with parameters:' "$dir" >&2
            printf ' %q' "${args[@]}" >&2
            printf '\n' >&2
            return 1
        fi
    )
}

wait_all() {
    local rc=0
    local pid
    for pid in "$@"; do
        if ! wait "$pid"; then
            rc=1
        fi
    done
    return "$rc"
}

# Separate env for the runs that need it.
run_test_matrix() {
    run_nextest ./test --release --features "extra_assertions size_classes" &
    p1=$!

    run_nextest ./test --release --features "size_classes" &
    p2=$!

    run_nextest ./test --release --features "static_array_backend extra_assertions size_classes" &
    p3=$!

    run_nextest ./test --release --features "static_array_backend size_classes" &
    p4=$!

    run_nextest ./test --release --features "extra_assertions" &
    p5=$!

    run_nextest ./test --release &
    p6=$!

    run_nextest ./test --release --features "static_array_backend extra_assertions" &
    p7=$!

    run_nextest ./test --release --features "static_array_backend" &
    p8=$!

    wait_all "$p1" "$p2" "$p3" "$p4" "$p5" "$p6" "$p7" "$p8"
}

run_nextest ./wee_alloc
run_test_matrix
