import http.server
import socketserver
import sys
import json

class MyHandler(http.server.SimpleHTTPRequestHandler):
    def do_POST(self):
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()
        response = {
            "result": 100,
            "error": None,
            "id": 1
        }
        self.wfile.write(json.dumps(response).encode('utf-8'))

    def log_message(self, format, *args):
        # Suppress logging to keep test output clean
        return

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 mock_server.py <port>")
        sys.exit(1)
    
    PORT = int(sys.argv[1])
    
    with socketserver.TCPServer(("", PORT), MyHandler) as httpd:
        print(f"Mock server listening on port {PORT}")
        httpd.serve_forever()