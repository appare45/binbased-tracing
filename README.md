# binbased-tracing

A process tracing and dynamic instrumentation tool for Linux/aarch64

## Overview

A tool that attaches to running processes and traces function execution. Uses ptrace and ELF analysis to dynamically inject trampoline code and collect timestamps at function entry points and returns.

## Key Features

- **Dynamic Instrumentation**: Trace function execution without modifying running binaries
- **Type-Safe Design**: Leverages Rust's type system to safely manage ptrace state transitions
- **Named Pipe Communication**: Trampoline code sends trace data via FIFO
- **Multiple Target Support**: Simultaneously instrument function entry points and all ret instructions

## System Requirements

- **Architecture**: aarch64 (ARM64)
- **OS**: Linux
- **Permissions**: CAP_SYS_PTRACE

## Build

```bash
cargo build --release
```

## Usage

### Launch and trace a new process

```bash
cargo run --release -- exec <PATH> [ARGS...]
```

Example: Trace the demo application
```bash
cargo run --release -- exec ./demo/demo
```

### Attach to an existing process

```bash
cargo run --release -- attach <PID>
```

### Testing with the demo application

Example of tracing a Go HTTP server:

```bash
# Terminal 1: Start the tracer
cargo run --release -- exec ./demo/demo

# Terminal 2: Send a request to trigger tracing
curl http://localhost:8080/
```

Testing with multiple requests:
```bash
for i in {1..5}; do
    curl -s http://localhost:8080/ >/dev/null 2>&1
    echo "Request $i sent"
    sleep 0.1
done
```

## Architecture

### Module Structure

```
src/
├── main.rs            # CLI entry point, overall orchestration
├── conf.rs            # Process lifecycle management
├── proc.rs            # Process information access
├── ptrace.rs          # ptrace wrapper (type-safe state management)
├── elf.rs             # ELF analysis and symbol extraction
├── maps.rs            # /proc/maps parser
├── symbol_analyzer.rs # Symbol analysis and ret instruction detection
├── instrument.rs      # Dynamic instrumentation
├── instruction.rs     # AArch64 instruction sequence construction
├── monitor.rs         # Process monitoring loop
├── pipe.rs            # Named pipe communication
└── error.rs           # Error type definitions
```

### Processing Flow

1. **Process Setup**: Launch new process or attach to existing one
2. **Symbol Analysis**: Detect target function addresses and ret instructions from ELF file
3. **Instrumentation Plan Creation**: Set up pipes, targets, and reader threads
4. **Instrumentation Execution**: Inject trampoline code
5. **Process Monitoring**: Collect and display trace data

### Type-Safe State Management

Ptrace states are represented by types:

- `Attached`: Attached, running
- `Stopped`: Stopped, register/memory operations allowed

Invalid state transitions result in compile-time errors.

## Trace Target

Currently traces a hardcoded symbol in `main.rs`:

```rust
const TARGET_SYMBOL: &str = "net/http.serverHandler.ServeHTTP";
```

This targets a Go HTTP handler, but will be made configurable in the future.

## Understanding Trace Output

```
[Entry] Goroutine 0x4000148380 entered at timestamp 1237188165626
[Completed] Goroutine 0x4000148380: entry=1237188165626, return=1237188167424, duration=1798 cycles
```

- **Timestamp**: CPU cycle counter value (`mrs x0, cntvct_el0`)
- **Duration**: CPU cycles taken for function execution
  - Small values (1000-5000): Fast execution, cache hits
  - Large values (>10000): I/O waits, context switches
- **Goroutine ID**: Go runtime goroutine identifier
