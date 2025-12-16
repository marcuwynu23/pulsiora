# Pulsiora - Client-Server CI/CD Pipeline Engine

Pulsiora is a production-ready CI/CD system that watches GitHub repositories and executes pipelines defined in custom `Pulsefile` DSL files.

## Architecture

The system is built as a Rust workspace with the following components:

- **pulsiora-core**: Core types and models for pipelines, steps, triggers, and events
- **pulsiora-parser**: Pulsefile DSL parser using pest
- **pulsiora-runner**: Pipeline execution engine
- **pulsiora-server**: HTTP server with GitHub webhook handler
- **pulsiora-client**: CLI client for interacting with the server

## Features

- ✅ Parse Pulsefile DSL with proper grammar-based parser
- ✅ Support for multiple Git event triggers (push, PR, merge, tag, release, branch create/delete)
- ✅ Branch filtering with wildcard and pattern support
- ✅ Ordered step execution
- ✅ Optional `allow_failure` flag for non-critical steps
- ✅ Multi-command, multi-language step execution
- ✅ GitHub webhook integration
- ✅ REST API for execution status and history
- ✅ Comprehensive test coverage

## Building

```bash
cargo build --release
```

## Running the Server

```bash
cd pulsiora-server
cargo run
```

The server will listen on `http://0.0.0.0:3000` by default.

## Using the Client CLI

```bash
# Check server health
cargo run --bin pulse -- health

# Generate Pulsefile template
cargo run --bin pulse -- init

# Register repository and upload Pulsefile
cargo run --bin pulse -- repo add <repo-url> --pulsefile Pulsefile

# Unregister repository
cargo run --bin pulse -- repo remove <repo-url>

# Check recent pipeline runs for a repository
cargo run --bin pulse -- pipeline status <repo>

# Fetch logs for a specific pipeline run
cargo run --bin pulse -- pipeline logs <repo> <run-id>

# List all pipeline executions
cargo run --bin pulse -- list

# Get execution status (deprecated, use pipeline logs)
cargo run --bin pulse -- status <execution-id>
```

## Pulsefile Format

See the example in the prompt above. A Pulsefile defines:

- Pipeline metadata (name, version)
- Git event triggers
- Ordered steps with commands and optional `allow_failure` flag

## Testing

Run all tests:

```bash
cargo test
```

Run tests for a specific crate:

```bash
cargo test -p pulsiora-core
cargo test -p pulsiora-parser
cargo test -p pulsiora-runner
cargo test -p pulsiora-server
```

## License

MIT

