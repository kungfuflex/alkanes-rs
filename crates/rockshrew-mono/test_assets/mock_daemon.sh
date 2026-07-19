#!/bin/sh

# Simple mock Bitcoin daemon using netcat.
# It listens on a given port and responds to a getblockcount RPC call.

PORT=$1

while true; do
  RESPONSE="HTTP/1.1 200 OK\r\nContent-Length: 34\r\nConnection: close\r\n\r\n{\"result\": 100, \"error\": null, \"id\": 1}"
  echo -e "$RESPONSE" | nc -l -p "$PORT"
done