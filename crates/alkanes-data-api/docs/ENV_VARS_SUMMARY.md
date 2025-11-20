# Environment Variables Summary

All environment variables for alkanes-data-api are configured in docker-compose files.
**No .env file is needed or used.**

## Configuration Location

- **Regtest**: `docker-compose.yaml`
- **Signet**: `docker-compose.signet.yaml`
- **Mainnet**: `docker-compose.mainnet.yaml`

## Variables Configured

### ✅ Fully Configured (No Changes Needed)

| Variable | Value | Purpose |
|----------|-------|---------|
| `DATABASE_URL` | `postgres://alkanes_user:alkanes_pass@postgres:5432/alkanes_indexer` | PostgreSQL connection |
| `REDIS_URL` | `redis://redis:6379` | Redis cache connection |
| `SANDSHREW_URL` | `http://jsonrpc:18888` | Unified Bitcoin+Metashrew RPC |
| `HOST` | `0.0.0.0` | Server bind address |
| `PORT` | `3000` | Server port |
| `RUST_LOG` | `info,alkanes_data_api=debug` | Logging level |

### 🔧 Network-Specific (Already Set Per Network)

| Variable | Regtest | Signet | Mainnet |
|----------|---------|--------|---------|
| `NETWORK_ENV` | `regtest` | `signet` | `mainnet` |
| `ALKANE_FACTORY_ID` | `"4:65522"` | `"0:0"` | `"840000:1"` |

### ⚠️ Needs User Configuration

| Variable | Default Value | What You Need |
|----------|---------------|---------------|
| `INFURA_ENDPOINT` | `https://mainnet.infura.io/v3/YOUR_INFURA_KEY_HERE` | Replace with valid Infura API key |

**How to get Infura key:**
1. Sign up at https://infura.io/ (free)
2. Create project
3. Copy Project ID
4. Update in docker-compose.yaml

**What happens without Infura:**
- ✅ 39 endpoints work perfectly
- ❌ 4 BTC price endpoints will return 500 error

## How to Update

Edit the docker-compose file directly:

```yaml
  alkanes-data-api:
    environment:
      # Update this line with your key:
      INFURA_ENDPOINT: https://mainnet.infura.io/v3/YOUR_PROJECT_ID
```

Then restart:
```bash
docker-compose restart alkanes-data-api
```

## Verification

Check what the running container sees:
```bash
docker-compose exec alkanes-data-api env | grep -E "DATABASE|REDIS|SANDSHREW|INFURA|PORT"
```

## No .env File Needed

The alkanes-data-api **does not use** a `.env` file. All configuration is done through docker-compose environment variables.

This is intentional to:
- ✅ Keep config in one place
- ✅ Avoid dotenv dependency issues  
- ✅ Make deployment simpler
- ✅ Support multiple networks easily
