#!/bin/bash
curl http://localhost:18888 -H 'Content-Type: application/json' -X POST -d '{"jsonrpc": "2.0", "id": 0, "method": "ord_blockcount", "params": []}'
