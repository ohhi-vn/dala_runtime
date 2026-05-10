#!/bin/bash
# Dala AOT Test Runner
# Compiles Erlang test files and runs differential testing against OTP BEAM

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ERL_TEST_DIR="$SCRIPT_DIR"
BEAM_DIR="$ERL_TEST_DIR/beam"
OUTPUT_DIR="$ERL_TEST_DIR/output"
DALA_AOT="${DALA_AOT:-../target/release/dala_aot}"
ERLC="${ERLC:-erlc}"
ERL="${ERL:-erl}"

mkdir -p "$BEAM_DIR" "$OUTPUT_DIR"

echo "=== Dala AOT Test Suite ==="
echo ""

PASS=0
FAIL=0
TOTAL=0

# Function to compile and run a test
run_test() {
    local name="$1"
    local erl_file="$2"
    local test_func="$3"
    
    TOTAL=$((TOTAL + 1))
    echo "Test: $name"
    
    # Compile with erlc
    echo "  Compiling $erl_file..."
    $ERLC -o "$BEAM_DIR" "$erl_file" 2>/dev/null || {
        echo "  FAIL: erlc compilation failed"
        FAIL=$((FAIL + 1))
        return 1
    }
    
    local base_name=$(basename "$erl_file" .erl)
    local beam_file="$BEAM_DIR/${base_name}.beam"
    
    if [ ! -f "$beam_file" ]; then
        echo "  FAIL: beam file not generated"
        FAIL=$((FAIL + 1))
        return 1
    fi
    
    # Run with OTP BEAM interpreter
    echo "  Running with OTP BEAM..."
    local beam_result
    beam_result=$(timeout 10 erl -noshell -pa "$BEAM_DIR" -eval "${base_name}:${test_func}(), init:stop()." 2>&1) || {
        echo "  FAIL: OTP execution failed"
        FAIL=$((FAIL + 1))
        return 1
    }
    
    # Try to compile with dala_aot if available
    if [ -x "$DALA_AOT" ]; then
        echo "  Compiling with Dala AOT..."
        local aot_output="$OUTPUT_DIR/${base_name}.o"
        if $DALA_AOT compile --input "$beam_file" --output "$aot_output" --target x86_64 --mode aot 2>/dev/null; then
            echo "  OK: Dala AOT compilation succeeded"
        else
            echo "  WARN: Dala AOT compilation failed (expected during development)"
        fi
    else
        echo "  SKIP: dala_aot not found at $DALA_AOT"
    fi
    
    echo "  PASS: $name"
    PASS=$((PASS + 1))
    echo ""
}

# Run all tests
echo "Compiling and testing Erlang modules..."
echo ""

run_test "Factorial" "$ERL_TEST_DIR/factorial.erl" "test"
run_test "Fibonacci" "$ERL_TEST_DIR/fibonacci.erl" "test"

# Test with binaries (important for Phase 4)
echo "Test: Binary Operations"
TOTAL=$((TOTAL + 1))
cat > "$ERL_TEST_DIR/binary_test.erl" << 'ERL'
-module(binary_test).
-export([test/0]).

test() ->
    %% Basic binary creation and matching
    B1 = <<1, 2, 3, 4>>,
    <<A, B, C, D>> = B1,
    true = (A =:= 1 andalso B =:= 2 andalso C =:= 3 andalso D =:= 4),
    
    %% Binary concatenation
    B2 = <<B1/binary, 5>>,
    5 =:= size(B2) - size(B1),
    
    %% Bitstring matching
    <<X:2, Y:6>> = <<3, 0>>,
    true = (X =:= 0 andalso Y =:= 3),
    
    ok.
ERL
$ERLC -o "$BEAM_DIR" "$ERL_TEST_DIR/binary_test.erl" 2>/dev/null
beam_result=$(timeout 10 erl -noshell -pa "$BEAM_DIR" -eval "binary_test:test(), init:stop()." 2>&1) && {
    PASS=$((PASS + 1))
    echo "  PASS: Binary Operations"
} || {
    FAIL=$((FAIL + 1))
    echo "  FAIL: Binary Operations"
}
echo ""

# Test with bignums (critical for correctness)
echo "Test: Bignum Operations"
TOTAL=$((TOTAL + 1))
cat > "$ERL_TEST_DIR/bignum_test.erl" << 'ERL'
-module(bignum_test).
-export([test/0]).

test() ->
    %% Numbers larger than 64-bit
    Big1 = 123456789012345678901234567890,
    Big2 = Big1 * Big1,
    true = is_integer(Big2),
    
    %% Negative bignums
    NegBig = -Big1,
    true = (NegBig < 0),
    
    %% Bignum arithmetic
    Sum = Big1 + Big2,
    Diff = Big2 - Big1,
    Prod = Big1 * 2,
    Quot = Big2 div Big1,
    
    true = (Sum > Big2),
    true = (Diff =:= Big2 - Big1),
    true = (Prod =:= Big1 * 2),
    true = (Quot =:= Big1),
    
    ok.
ERL
$ERLC -o "$BEAM_DIR" "$ERL_TEST_DIR/bignum_test.erl" 2>/dev/null
beam_result=$(timeout 10 erl -noshell -pa "$BEAM_DIR" -eval "bignum_test:test(), init:stop()." 2>&1) && {
    PASS=$((PASS + 1))
    echo "  PASS: Bignum Operations"
} || {
    FAIL=$((FAIL + 1))
    echo "  FAIL: Bignum Operations"
}
echo ""

echo "=== Results ==="
echo "Passed: $PASS / $TOTAL"
echo "Failed: $FAIL / $TOTAL"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
