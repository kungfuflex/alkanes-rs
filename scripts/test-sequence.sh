#!/bin/bash
curl http://localhost:8081 -X POST -d '{"jsonrpc": "2.0", "method": "metashrew_view", "params": ["sequence", "0x", "latest"], "id": 0}' -H 'Content-Type: application/json'
