# Debshrew-Mono: Change Data Capture for Bitcoin Indexing

Debshrew-Mono extends [rockshrew-mono](../rockshrew-mono) with Change Data Capture (CDC) functionality, enabling real-time streaming of Bitcoin indexing data to external systems via Kafka and Debezium-compatible events.

## Overview

Debshrew-Mono provides a non-invasive wrapper around the existing Metashrew indexing infrastructure that captures key-value updates during Bitcoin block processing and streams them as structured CDC events. This enables real-time data pipelines, analytics, and integration with downstream systems.

### Key Features

- **Real-time CDC**: Stream Bitcoin indexing data changes as they happen
- **Debezium Compatibility**: Standard CDC event format for seamless integration
- **Configurable Relationships**: Map key-value pairs to relational table structures
- **High Performance**: Minimal overhead on existing indexing operations
- **Reliable Delivery**: Kafka producer with retry logic and error handling
- **Schema Management**: Avro schema support with optional schema registry integration
- **Flexible Filtering**: Regex-based key pattern matching for selective streaming

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Bitcoin Node  │───▶│  Debshrew-Mono   │───▶│  Kafka Cluster  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │                          │
                              ▼                          ▼
                       ┌──────────────┐         ┌─────────────────┐
                       │   RocksDB    │         │   Downstream    │
                       │   Storage    │         │    Systems      │
                       └──────────────┘         └─────────────────┘
```

Debshrew-Mono sits between the Bitcoin indexing process and storage, intercepting key-value updates and streaming them to Kafka while maintaining full compatibility with existing Metashrew functionality.

## Quick Start

### 1. Installation

Add debshrew-mono to your workspace:

```toml
# Cargo.toml
[workspace]
members = [
    "crates/debshrew-mono",
    # ... other crates
]
```

### 2. Configuration

Create a CDC configuration file (see [`examples/cdc-config.yaml`](examples/cdc-config.yaml)):

```yaml
# Basic CDC configuration
global_settings:
  max_buffer_size: 1000
  flush_interval_ms: 1000

kafka_config:
  bootstrap_servers: "localhost:9092"
  topic_prefix: "bitcoin.cdc"

relationships:
  - name: "bitcoin_addresses"
    key_pattern: "^address:([13][a-km-zA-HJ-NP-Z1-9]{25,34})$"
    table_name: "addresses"
    operation_type: "CreateUpdate"
    # ... field mappings
```

### 3. Running

Start debshrew-mono with CDC enabled:

```bash
# Basic usage
cargo run --bin debshrew-mono -- \
  --bitcoin-rpc-url http://localhost:8332 \
  --bitcoin-rpc-user bitcoin \
  --bitcoin-rpc-password password \
  --cdc-config ./cdc-config.yaml

# With additional options
cargo run --bin debshrew-mono -- \
  --bitcoin-rpc-url http://localhost:8332 \
  --bitcoin-rpc-user bitcoin \
  --bitcoin-rpc-password password \
  --cdc-config ./cdc-config.yaml \
  --cdc-dry-run \
  --cdc-buffer-size 2000 \
  --cdc-flush-interval 500
```

## Configuration

### Global Settings

```yaml
global_settings:
  max_buffer_size: 1000        # Maximum events to buffer before flushing
  flush_interval_ms: 1000      # Maximum time to wait before flushing
  retry_attempts: 3            # Number of retry attempts for failed sends
  retry_backoff_ms: 1000       # Backoff time between retries
  enable_deduplication: true   # Enable event deduplication
  include_block_metadata: true # Include block height/hash in events
```

### Kafka Configuration

```yaml
kafka_config:
  bootstrap_servers: "localhost:9092"
  topic_prefix: "bitcoin.cdc"
  acks: "all"                  # Acknowledgment level
  compression: "gzip"          # Compression algorithm
  batch_size: 1000            # Batch size for producer
  linger_ms: 100              # Linger time for batching
  additional_properties:       # Additional Kafka properties
    "security.protocol": "PLAINTEXT"
    "client.id": "debshrew-mono"
```

### CDC Relationships

Define how key-value pairs map to relational table structures:

```yaml
relationships:
  - name: "bitcoin_transactions"
    key_pattern: "^tx:([a-fA-F0-9]{64})$"
    table_name: "transactions"
    operation_type: "CreateOnly"
    key_mapping:
      primary_key_fields: ["txid"]
      extraction_pattern: "^tx:(.+)$"
    value_mapping:
      value_format: "Json"
      fields:
        - name: "txid"
          path: "$.txid"
          data_type: "varchar"
          nullable: false
        - name: "block_height"
          path: "$.height"
          data_type: "integer"
          nullable: false
```

#### Operation Types

- **CreateOnly**: Only emit events for new keys (INSERT operations)
- **CreateUpdate**: Emit events for new and updated keys (INSERT/UPDATE operations)
- **CreateUpdateDelete**: Emit events for all operations including deletions

#### Value Formats

- **Json**: Parse value as JSON and extract fields using JSONPath
- **Raw**: Treat value as raw bytes and include metadata

## Event Format

Debshrew-Mono generates Debezium-compatible CDC events:

```json
{
  "schema": {
    "type": "struct",
    "fields": [
      {"field": "before", "type": "struct", "optional": true},
      {"field": "after", "type": "struct", "optional": true},
      {"field": "source", "type": "struct", "optional": false},
      {"field": "op", "type": "string", "optional": false},
      {"field": "ts_ms", "type": "int64", "optional": false}
    ]
  },
  "payload": {
    "before": null,
    "after": {
      "txid": "abc123...",
      "block_height": 800000,
      "fee": 1000
    },
    "source": {
      "version": "1.0.0",
      "connector": "debshrew-mono",
      "name": "bitcoin",
      "ts_ms": 1672531200000,
      "db": "bitcoin",
      "table": "transactions",
      "block_height": 800000,
      "block_hash": "def456..."
    },
    "op": "c",
    "ts_ms": 1672531200000
  }
}
```

## Command Line Options

```
USAGE:
    debshrew-mono [OPTIONS]

OPTIONS:
    --cdc-config <FILE>              Path to CDC configuration file
    --cdc-dry-run                    Enable dry-run mode (no Kafka publishing)
    --cdc-buffer-size <SIZE>         Override buffer size from config
    --cdc-flush-interval <MS>        Override flush interval from config
    --cdc-kafka-servers <SERVERS>    Override Kafka bootstrap servers
    --cdc-topic-prefix <PREFIX>      Override Kafka topic prefix
    
    # Standard rockshrew-mono options
    --bitcoin-rpc-url <URL>          Bitcoin RPC URL
    --bitcoin-rpc-user <USER>        Bitcoin RPC username
    --bitcoin-rpc-password <PASS>    Bitcoin RPC password
    --data-dir <DIR>                 Data directory for RocksDB
    --json-rpc-port <PORT>           JSON-RPC server port
    # ... other rockshrew-mono options
```

## Integration Examples

### Kafka Connect + PostgreSQL

Stream Bitcoin data to PostgreSQL using Kafka Connect:

```json
{
  "name": "bitcoin-postgres-sink",
  "config": {
    "connector.class": "io.confluent.connect.jdbc.JdbcSinkConnector",
    "tasks.max": "1",
    "topics.regex": "bitcoin\\.cdc\\..*",
    "connection.url": "jdbc:postgresql://localhost:5432/bitcoin",
    "connection.user": "postgres",
    "connection.password": "password",
    "auto.create": "true",
    "auto.evolve": "true",
    "insert.mode": "upsert",
    "pk.mode": "record_key",
    "transforms": "unwrap",
    "transforms.unwrap.type": "io.debezium.transforms.ExtractNewRecordState"
  }
}
```

### Elasticsearch Integration

Stream to Elasticsearch for search and analytics:

```json
{
  "name": "bitcoin-elasticsearch-sink",
  "config": {
    "connector.class": "io.confluent.connect.elasticsearch.ElasticsearchSinkConnector",
    "tasks.max": "1",
    "topics.regex": "bitcoin\\.cdc\\..*",
    "connection.url": "http://localhost:9200",
    "type.name": "_doc",
    "key.ignore": "false",
    "schema.ignore": "true",
    "transforms": "unwrap",
    "transforms.unwrap.type": "io.debezium.transforms.ExtractNewRecordState"
  }
}
```

## Performance Considerations

### Buffer Management

- **Buffer Size**: Larger buffers reduce Kafka overhead but increase memory usage
- **Flush Interval**: Shorter intervals reduce latency but increase Kafka load
- **Batch Size**: Kafka producer batching improves throughput

### Memory Usage

- CDC tracking adds minimal memory overhead (~1-2% of total indexing memory)
- Event buffers are bounded and automatically flushed
- Schema caching uses LRU eviction to prevent unbounded growth

### Throughput

- Typical overhead: 5-10% additional CPU usage during block processing
- Kafka producer is asynchronous and doesn't block indexing
- Regex matching is optimized with compiled patterns

## Monitoring

### Metrics

Debshrew-Mono exposes metrics for monitoring CDC performance:

- `cdc_events_generated_total`: Total CDC events generated
- `cdc_events_sent_total`: Total events successfully sent to Kafka
- `cdc_events_failed_total`: Total events that failed to send
- `cdc_buffer_size_current`: Current buffer size
- `cdc_flush_duration_seconds`: Time spent flushing events

### Logging

Configure logging levels for different components:

```bash
RUST_LOG=debshrew_mono=info,debshrew_mono::cdc_tracker=debug cargo run --bin debshrew-mono
```

## Development

### Building

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run --bin debshrew-mono -- --help
```

### Testing

```bash
# Unit tests
cargo test --lib

# Integration tests (requires Kafka)
cargo test --test integration

# CDC-specific tests
cargo test --package debshrew-mono
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## Troubleshooting

### Common Issues

**Kafka Connection Errors**
```
Error: Failed to connect to Kafka: BrokerTransportFailure
```
- Verify Kafka is running and accessible
- Check `bootstrap_servers` configuration
- Verify network connectivity and firewall settings

**Schema Validation Errors**
```
Error: Schema validation failed for relationship 'bitcoin_addresses'
```
- Check JSONPath expressions in field mappings
- Verify value format matches actual data structure
- Enable debug logging to see raw values

**High Memory Usage**
```
Warning: CDC buffer size approaching limit
```
- Reduce `max_buffer_size` or `flush_interval_ms`
- Check Kafka producer performance
- Monitor downstream consumer lag

### Debug Mode

Enable comprehensive debugging:

```bash
RUST_LOG=debshrew_mono=trace cargo run --bin debshrew-mono -- \
  --cdc-config ./cdc-config.yaml \
  --cdc-dry-run
```

## License

This project is licensed under the same terms as the parent Metashrew project.

## Related Projects

- [rockshrew-mono](../rockshrew-mono): Core Bitcoin indexing functionality
- [metashrew-runtime](../metashrew-runtime): WASM runtime for Bitcoin indexing
- [Debezium](https://debezium.io/): Open-source CDC platform
- [Apache Kafka](https://kafka.apache.org/): Distributed streaming platform