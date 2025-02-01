#!/bin/bash

# Define log directories
LOG_DIR="logs"
mkdir -p "$LOG_DIR"

# Create data directories for trin instances
DATA_DIR_SENDER="./data_sender"
DATA_DIR_RECEIVER="./data_receiver"
mkdir -p "$LOG_DIR/$DATA_DIR_SENDER" "$LOG_DIR/$DATA_DIR_RECEIVER"

# Clone portal-accumulators repository if not already present
if [ ! -d "../../portal-accumulators" ]; then
    git clone https://github.com/ethereum/portal-accumulators ../../portal-accumulators || { echo "Failed to clone portal-accumulators"; exit 1; }
fi

# Build trin-benchmark-cordinator with release profile
pushd ../.. || { echo "Failed to change directory"; exit 1; }
cargo build --release -p trin-bench || { echo "Failed to build trin-benchmark-cordinator"; exit 1; }
popd || { echo "Failed to return to original directory"; exit 1; }

# Define process PIDs
PIDS=()

# Find available ports dynamically and ensure they are unique
find_unused_port() {
    local port=$1
    while ss -tuln | awk '{print $4}' | grep -q ":$port$"; do
        port=$((port + 1))
    done
    echo $port
}

PORT_SENDER=$(find_unused_port 9050)
PORT_RECEIVER=$(find_unused_port $((PORT_SENDER + 10)))
EXT_PORT_SENDER=$(find_unused_port 9100)
EXT_PORT_RECEIVER=$(find_unused_port $((EXT_PORT_SENDER + 10)))

# Run trin sender with perf profiling
cargo flamegraph --profile profiling -c "record -F 97 --call-graph dwarf,64000 -g -o $LOG_DIR/trin_sender.perf" --root --output "$LOG_DIR/trin_sender.svg" -p trin -- \
    --web3-transport http \
    --web3-http-address http://127.0.0.1:$PORT_SENDER/ \
    --mb 0 \
    --bootnodes none \
    --external-address 127.0.0.1:$EXT_PORT_SENDER \
    --discovery-port $EXT_PORT_SENDER \
    --data-dir "$LOG_DIR/$DATA_DIR_SENDER" \
    > "$LOG_DIR/trin_sender.log" 2>&1 &
PIDS+=("$!")

# Run trin receiver with perf profiling
cargo flamegraph --profile profiling -c "record -F 97 --call-graph dwarf,64000 -g -o $LOG_DIR/trin_receiver.perf" --root --output "$LOG_DIR/trin_receiver.svg" -p trin -- \
    --web3-transport http \
    --web3-http-address http://127.0.0.1:$PORT_RECEIVER/ \
    --mb 10000 \
    --bootnodes none \
    --external-address 127.0.0.1:$EXT_PORT_RECEIVER \
    --discovery-port $EXT_PORT_RECEIVER \
    --data-dir "$LOG_DIR/$DATA_DIR_RECEIVER" \
    > "$LOG_DIR/trin_receiver.log" 2>&1 &
PIDS+=("$!")

# Run trin benchmark coordinator without perf profiling
../../target/release/trin-bench \
    --web3-http-address-node-1 http://127.0.0.1:$PORT_SENDER/ \
    --web3-http-address-node-2 http://127.0.0.1:$PORT_RECEIVER/ \
    --epoch-accumulator-path ../../portal-accumulators \
    --start-era1 1000 \
    --end-era1 1002 \
    > "$LOG_DIR/trin_benchmark.log" 2>&1 &
TRIN_BENCH_PID=$!

echo "Started Benchmark"

CLEANED_UP=false
# Function to clean up processes on SIGINT and SIGTERM
cleanup() {
    if $CLEANED_UP; then
        return
    fi
    CLEANED_UP=true
    echo "Finished benchmark. Stopping processes..."
    
    # Stop trin sender and receiver
    for PID in "${PIDS[@]}"; do
        if kill -0 "$PID" 2>/dev/null; then
            echo "Killing process with PID $PID..."
            pkill -SIGINT -P "$PID"
        fi
    done
    
    # Wait for trin sender and receiver to finish
    for PID in "${PIDS[@]}"; do
        if kill -0 "$PID" 2>/dev/null; then
            echo "Waiting process with PID $PID..."
            wait "$PID" 2>/dev/null
        fi
    done
    
    # Stop trin-bench process separately
    if kill -0 "$TRIN_BENCH_PID" 2>/dev/null; then
        echo "Stopping trin-bench with PID $TRIN_BENCH_PID..."
        kill -SIGINT "$TRIN_BENCH_PID"
        wait "$TRIN_BENCH_PID" 2>/dev/null
    fi

    echo "All processes stopped."

    # Remove data directories
    sudo rm -rf "$LOG_DIR/$DATA_DIR_SENDER" "$LOG_DIR/$DATA_DIR_RECEIVER" "$LOG_DIR/trin_sender.perf" "$LOG_DIR/trin_receiver.perf"
}

# Trap signals
trap cleanup SIGINT SIGTERM ERR

# Wait for trin-bench to finish
wait "$TRIN_BENCH_PID"

# unset the trap
trap - SIGINT SIGTERM ERR

cleanup
