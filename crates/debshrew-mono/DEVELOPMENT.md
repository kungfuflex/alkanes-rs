# Debshrew-Mono Development Guide

This guide covers development setup, testing, and deployment for debshrew-mono.

## Development Environment Setup

### Prerequisites

- Rust 1.75+ with Cargo
- Docker and Docker Compose
- Git

### Local Development Setup

1. **Clone the repository and navigate to the project:**
   ```bash
   git clone <repository-url>
   cd metashrew/crates/debshrew-mono
   ```

2. **Start the development infrastructure:**
   ```bash
   docker-compose up -d kafka zookeeper schema-registry postgres
   ```

3. **Wait for services to be ready:**
   ```bash
   # Check service health
   docker-compose ps
   
   # Wait for Kafka to be ready
   docker-compose exec kafka kafka-topics --bootstrap-server localhost:9092 --list
   ```

4. **Build the project:**
   ```bash
   cargo build
   ```

5. **Run tests:**
   ```bash
   cargo test
   ```

### Development Workflow

#### Running debshrew-mono locally

```bash
# Basic run with CDC enabled
cargo run --bin debshrew-mono -- \
  --bitcoin-rpc-url http://localhost:18332 \
  --bitcoin-rpc-user bitcoin \
  --bitcoin-rpc-password password \
  --cdc-config ./examples/cdc-config.yaml \
  --cdc-dry-run

# With debug logging
RUST_LOG=debshrew_mono=debug cargo run --bin debshrew-mono -- \
  --bitcoin-rpc-url http://localhost:18332 \
  --bitcoin-rpc-user bitcoin \
  --bitcoin-rpc-password password \
  --cdc-config ./examples/cdc-config.yaml
```

#### Testing CDC functionality

1. **Start Bitcoin Core in regtest mode:**
   ```bash
   docker-compose up -d bitcoin-core
   ```

2. **Generate some test blocks:**
   ```bash
   # Connect to Bitcoin Core
   docker-compose exec bitcoin-core bitcoin-cli -regtest -rpcuser=bitcoin -rpcpassword=password getblockchaininfo
   
   # Generate blocks
   docker-compose exec bitcoin-core bitcoin-cli -regtest -rpcuser=bitcoin -rpcpassword=password generatetoaddress 10 bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh
   ```

3. **Monitor Kafka topics:**
   ```bash
   # List topics
   docker-compose exec kafka kafka-topics --bootstrap-server localhost:9092 --list
   
   # Monitor CDC events
   docker-compose exec kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic bitcoin.cdc.addresses --from-beginning
   ```

4. **Check PostgreSQL for synced data:**
   ```bash
   docker-compose exec postgres psql -U postgres -d bitcoin -c "SELECT * FROM addresses LIMIT 10;"
   ```

### Code Organization

```
crates/debshrew-mono/
├── src/
│   ├── lib.rs                    # Main library exports
│   ├── main.rs                   # Binary entry point
│   ├── cdc_config.rs            # Configuration management
│   ├── cdc_tracker.rs           # Core CDC functionality
│   ├── kafka_producer.rs        # Kafka integration
│   ├── relationship_matcher.rs  # Pattern matching
│   ├── runtime_adapter.rs       # Metashrew runtime integration
│   └── schema_manager.rs        # Schema management
├── tests/
│   └── integration_test.rs      # Integration tests
├── examples/
│   ├── cdc-config.yaml         # Example configuration
│   ├── postgres-init.sql       # Database schema
│   └── bitcoin.conf            # Bitcoin Core config
├── docker-compose.yml          # Development infrastructure
├── Dockerfile                  # Production container
└── README.md                   # User documentation
```

### Testing Strategy

#### Unit Tests
```bash
# Run all unit tests
cargo test --lib

# Run specific module tests
cargo test --lib cdc_config
cargo test --lib relationship_matcher
```

#### Integration Tests
```bash
# Run integration tests (requires Kafka)
cargo test --test integration_test

# Run with logging
RUST_LOG=debug cargo test --test integration_test -- --nocapture
```

#### End-to-End Testing
```bash
# Start full infrastructure
docker-compose up -d

# Run debshrew-mono with test configuration
cargo run --bin debshrew-mono -- \
  --bitcoin-rpc-url http://localhost:18332 \
  --bitcoin-rpc-user bitcoin \
  --bitcoin-rpc-password password \
  --cdc-config ./examples/cdc-config.yaml

# Generate test data and verify CDC pipeline
./scripts/e2e-test.sh
```

## Configuration Management

### CDC Configuration Structure

The CDC configuration defines how key-value updates are transformed into relational CDC events:

```yaml
global_settings:
  max_buffer_size: 1000          # Events to buffer before flushing
  flush_interval_ms: 1000        # Maximum flush interval
  retry_attempts: 3              # Retry attempts for failed sends
  enable_deduplication: true     # Enable event deduplication

kafka_config:
  bootstrap_servers: "localhost:9092"
  topic_prefix: "bitcoin.cdc"
  # ... additional Kafka settings

relationships:
  - name: "bitcoin_addresses"
    key_pattern: "^address:([13][a-km-zA-HJ-NP-Z1-9]{25,34})$"
    table_name: "addresses"
    operation_type: "CreateUpdate"
    # ... field mappings
```

### Environment Variables

```bash
# Logging
export RUST_LOG=debshrew_mono=info,debshrew_mono::cdc_tracker=debug

# Bitcoin RPC
export BITCOIN_RPC_URL=http://localhost:18332
export BITCOIN_RPC_USER=bitcoin
export BITCOIN_RPC_PASSWORD=password

# Kafka
export KAFKA_BOOTSTRAP_SERVERS=localhost:9092
export KAFKA_TOPIC_PREFIX=bitcoin.cdc

# Data directory
export DATA_DIR=/path/to/data
```

## Performance Tuning

### Memory Management

1. **Buffer Size Tuning:**
   ```yaml
   global_settings:
     max_buffer_size: 2000        # Increase for higher throughput
     flush_interval_ms: 500       # Decrease for lower latency
   ```

2. **Kafka Producer Tuning:**
   ```yaml
   kafka_config:
     batch_size: 2000            # Larger batches for throughput
     linger_ms: 100              # Batch timeout
     compression: "gzip"         # Reduce network usage
   ```

### RocksDB Optimization

Debshrew-mono inherits RocksDB settings from rockshrew-mono. For CDC workloads:

```bash
# Increase write buffer for high-throughput scenarios
--rocksdb-write-buffer-size 128MB
--rocksdb-max-write-buffer-number 4
```

### Monitoring and Metrics

#### Application Metrics

Debshrew-mono exposes Prometheus-compatible metrics:

- `cdc_events_generated_total`: Total CDC events generated
- `cdc_events_sent_total`: Total events sent to Kafka
- `cdc_events_failed_total`: Total failed events
- `cdc_buffer_size_current`: Current buffer size
- `cdc_flush_duration_seconds`: Flush operation duration

#### Kafka Monitoring

Monitor Kafka performance:

```bash
# Check topic lag
docker-compose exec kafka kafka-consumer-groups --bootstrap-server localhost:9092 --describe --all-groups

# Monitor topic throughput
docker-compose exec kafka kafka-run-class kafka.tools.ConsumerPerformance \
  --bootstrap-server localhost:9092 \
  --topic bitcoin.cdc.addresses \
  --messages 1000
```

#### Database Monitoring

Monitor PostgreSQL performance:

```sql
-- Check CDC table sizes
SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
FROM pg_tables 
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

-- Monitor CDC health
SELECT * FROM cdc_health;
```

## Debugging

### Common Issues

#### Kafka Connection Problems

```bash
# Check Kafka connectivity
docker-compose exec kafka kafka-broker-api-versions --bootstrap-server localhost:9092

# Verify topic creation
docker-compose exec kafka kafka-topics --bootstrap-server localhost:9092 --list | grep bitcoin.cdc
```

#### Schema Validation Errors

```bash
# Enable debug logging for schema validation
RUST_LOG=debshrew_mono::schema_manager=debug cargo run --bin debshrew-mono

# Test JSONPath expressions
echo '{"address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", "balance": 5000000000}' | jq '$.address'
```

#### Performance Issues

```bash
# Monitor buffer usage
RUST_LOG=debshrew_mono::cdc_tracker=debug cargo run --bin debshrew-mono

# Check Kafka producer metrics
docker-compose exec kafka kafka-run-class kafka.tools.JmxTool \
  --object-name kafka.producer:type=producer-metrics,client-id=debshrew-mono
```

### Debug Mode

Enable comprehensive debugging:

```bash
# Full debug logging
RUST_LOG=trace cargo run --bin debshrew-mono -- \
  --cdc-config ./examples/cdc-config.yaml \
  --cdc-dry-run

# CDC-specific debugging
RUST_LOG=debshrew_mono::cdc_tracker=trace,debshrew_mono::kafka_producer=debug \
  cargo run --bin debshrew-mono
```

## Deployment

### Production Deployment

#### Docker Deployment

1. **Build production image:**
   ```bash
   docker build -t debshrew-mono:latest .
   ```

2. **Deploy with Docker Compose:**
   ```bash
   # Production docker-compose.yml
   docker-compose -f docker-compose.prod.yml up -d
   ```

3. **Configure for production:**
   ```yaml
   # Production CDC configuration
   global_settings:
     max_buffer_size: 5000
     flush_interval_ms: 1000
     retry_attempts: 5
     retry_backoff_ms: 2000
   
   kafka_config:
     bootstrap_servers: "kafka-cluster:9092"
     acks: "all"
     compression: "gzip"
     batch_size: 2000
   ```

#### Kubernetes Deployment

```yaml
# debshrew-mono-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: debshrew-mono
spec:
  replicas: 1
  selector:
    matchLabels:
      app: debshrew-mono
  template:
    metadata:
      labels:
        app: debshrew-mono
    spec:
      containers:
      - name: debshrew-mono
        image: debshrew-mono:latest
        ports:
        - containerPort: 3000
        env:
        - name: RUST_LOG
          value: "debshrew_mono=info"
        - name: BITCOIN_RPC_URL
          value: "http://bitcoin-core:8332"
        volumeMounts:
        - name: data
          mountPath: /app/data
        - name: config
          mountPath: /app/cdc-config.yaml
          subPath: cdc-config.yaml
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: debshrew-data
      - name: config
        configMap:
          name: debshrew-config
```

### Monitoring in Production

#### Health Checks

```bash
# Application health
curl http://localhost:3000/health

# Kafka connectivity
curl http://localhost:3000/health/kafka

# Database connectivity
curl http://localhost:3000/health/db
```

#### Alerting

Set up alerts for:

- CDC event processing lag
- Kafka connection failures
- Buffer overflow conditions
- Schema validation errors
- Database connection issues

### Backup and Recovery

#### Data Backup

```bash
# Backup RocksDB data
tar -czf debshrew-backup-$(date +%Y%m%d).tar.gz /app/data

# Backup PostgreSQL
docker-compose exec postgres pg_dump -U postgres bitcoin > bitcoin-backup-$(date +%Y%m%d).sql
```

#### Disaster Recovery

1. **Stop debshrew-mono**
2. **Restore RocksDB data from backup**
3. **Restore PostgreSQL from backup**
4. **Restart debshrew-mono**
5. **Verify CDC pipeline integrity**

## Contributing

### Development Guidelines

1. **Code Style:**
   - Follow Rust standard formatting (`cargo fmt`)
   - Use `cargo clippy` for linting
   - Add comprehensive documentation

2. **Testing:**
   - Write unit tests for all new functionality
   - Add integration tests for CDC features
   - Ensure all tests pass before submitting

3. **Documentation:**
   - Update README.md for user-facing changes
   - Update this development guide for internal changes
   - Add inline code documentation

### Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Implement changes with tests
4. Update documentation
5. Submit pull request with detailed description

### Release Process

1. **Version Bump:**
   ```bash
   # Update Cargo.toml version
   cargo update
   ```

2. **Testing:**
   ```bash
   # Full test suite
   cargo test --all
   
   # Integration tests
   docker-compose up -d
   cargo test --test integration_test
   ```

3. **Build and Tag:**
   ```bash
   # Build release
   cargo build --release
   
   # Tag release
   git tag v1.0.0
   git push origin v1.0.0
   ```

4. **Deploy:**
   ```bash
   # Build Docker image
   docker build -t debshrew-mono:v1.0.0 .
   
   # Push to registry
   docker push debshrew-mono:v1.0.0
   ```

## Troubleshooting

### Performance Issues

1. **High Memory Usage:**
   - Reduce `max_buffer_size`
   - Increase `flush_interval_ms`
   - Monitor Kafka consumer lag

2. **High CPU Usage:**
   - Optimize regex patterns in relationships
   - Reduce CDC relationship complexity
   - Enable Kafka compression

3. **Network Issues:**
   - Check Kafka connectivity
   - Verify DNS resolution
   - Monitor network latency

### Data Consistency Issues

1. **Missing CDC Events:**
   - Check buffer overflow logs
   - Verify Kafka topic configuration
   - Monitor producer acknowledgments

2. **Duplicate Events:**
   - Enable deduplication in configuration
   - Check Kafka exactly-once semantics
   - Verify consumer idempotency

3. **Schema Validation Errors:**
   - Validate JSONPath expressions
   - Check data type compatibility
   - Monitor schema evolution

For additional support, check the main project documentation or open an issue in the repository.