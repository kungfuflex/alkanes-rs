# Remaining Implementation Tasks

## Completed ✅
1. Created all service modules:
   - `services/alkanes.rs` - Alkanes RPC operations
   - `services/pools.rs` - Pool database queries
   - `services/history.rs` - Transaction history queries
   - `services/bitcoin.rs` - Bitcoin/UTXO operations
   - `services/alkanes_rpc.rs` - RPC client wrapper

2. Updated alkanes handlers (all 6 endpoints)
3. Started updating pools handlers

## Remaining Tasks

### 1. Complete Pool Handlers (`handlers/pools.rs`)

Update these functions:
- `get_pool_details` - Call `pool_service.get_pool_by_id()`
- `get_all_pools_details` - Call `pool_service.get_pools_by_factory()` with metrics
- `address_positions` - Call `pool_service.get_address_positions()`
- `get_all_token_pairs` - Call `pool_service.get_all_token_pairs()`
- `get_token_pairs` - Call `pool_service.get_token_pairs()`
- `get_alkane_swap_pair_details` - Calculate swap paths

### 2. Complete History Handlers (`handlers/history.rs`)

Update all 18 functions to call `HistoryService` methods:
- `get_pool_swap_history`
- `get_token_swap_history`  
- `get_pool_mint_history`
- `get_pool_burn_history`
- `get_pool_creation_history`
- `get_address_swap_history_for_pool`
- `get_address_swap_history_for_token`
- `get_address_wrap_history`
- `get_address_unwrap_history`
- `get_all_wrap_history`
- `get_all_unwrap_history`
- `get_total_unwrap_amount`
- `get_address_pool_creation_history`
- `get_address_pool_mint_history`
- `get_address_pool_burn_history`
- `get_all_address_amm_tx_history`
- `get_all_amm_tx_history`

### 3. Complete Bitcoin Handlers (`handlers/bitcoin.rs`)

Update all 7 functions to call `BitcoinService` methods:
- `get_address_balance`
- `get_taproot_balance`
- `get_address_utxos`
- `get_account_utxos`
- `get_account_balance`
- `get_taproot_history`
- `get_intent_history`

### 4. Fix Compilation Issues

Run `cargo check` and fix:
- Missing imports
- Type mismatches
- Unused variables
- Missing trait implementations

### 5. Add Missing Cargo Dependencies

May need to add:
```toml
bigdecimal = "0.4"  # For numeric calculations
```

### 6. Test Compilation

```bash
cargo build --release -p alkanes-data-api
```

### 7. Create Dockerfile

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p alkanes-data-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates
COPY --from=builder /app/target/release/alkanes-data-api /usr/local/bin/
CMD ["alkanes-data-api"]
```

### 8. Update Documentation

- README.md - Update with final API endpoints
- QUICKSTART.md - Update environment variables
- Update docker-compose examples

## Handler Implementation Pattern

All handlers follow this pattern:

```rust
pub async fn handler_name(
    state: web::Data<AppState>,
    req: web::Json<RequestType>,
) -> impl Responder {
    // 1. Validate required fields
    if req.required_field.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "required_field is required".to_string(),
        ));
    }

    // 2. Create service instance
    let service = ServiceType::new(
        state.db_pool.clone(),  // or state.alkanes_rpc.clone()
        state.redis_client.clone(),
        // ... other dependencies
    );

    // 3. Call service method
    match service.method_name(&req.param).await {
        Ok(result) => {
            let response = ApiResponse::ok(result);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to...".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
```

## Quick Implementation Script

To complete the remaining handlers, follow this approach for each file:

1. **pools.rs**: Replace all `// TODO` sections with service calls
2. **history.rs**: Replace all `// TODO` sections with service calls  
3. **bitcoin.rs**: Replace all `// TODO` sections with service calls

Each handler just needs:
- Service instantiation
- Parameter validation
- Service method call
- Error handling

## Testing

After implementation:

1. **Start services**:
```bash
# Terminal 1: PostgreSQL
docker-compose up postgres

# Terminal 2: Redis
docker-compose up redis

# Terminal 3: Sandshrew
./sandshrew

# Terminal 4: API
RUST_LOG=info cargo run --release -p alkanes-data-api
```

2. **Test endpoints**:
```bash
# Health check
curl http://localhost:3000/api/v1/health

# Get BTC price
curl -X POST http://localhost:3000/api/v1/get-bitcoin-price \
  -H "Content-Type: application/json"

# Get alkanes
curl -X POST http://localhost:3000/api/v1/get-alkanes \
  -H "Content-Type: application/json" \
  -d '{"limit": 10, "offset": 0}'
```

## Performance Optimizations

After basic implementation works:

1. Add connection pooling tuning
2. Implement Redis caching for expensive queries
3. Add database indexes for common query patterns
4. Implement background cache warming
5. Add request rate limiting
6. Add response compression

## Security Considerations

1. Validate all user inputs
2. Use prepared statements (sqlx does this automatically)
3. Set reasonable limits on pagination
4. Add timeout for RPC calls
5. Sanitize error messages (don't leak internal info)

## Deployment Checklist

- [ ] All handlers implemented
- [ ] Compilation successful
- [ ] Basic integration tests pass
- [ ] Dockerfile created
- [ ] Environment variables documented
- [ ] Added to docker-compose files
- [ ] README updated
- [ ] Performance tested with load tool
