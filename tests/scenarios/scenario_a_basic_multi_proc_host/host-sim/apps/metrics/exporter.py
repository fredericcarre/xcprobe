#!/usr/bin/env python3
"""Mock metrics exporter for testing xcprobe."""

import os
import time
import logging
from http.server import HTTPServer, BaseHTTPRequestHandler

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class MetricsHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/metrics':
            self.send_response(200)
            self.send_header('Content-Type', 'text/plain')
            self.end_headers()
            metrics = """# HELP requests_total Total requests
# TYPE requests_total counter
requests_total 100
# HELP uptime_seconds Uptime in seconds
# TYPE uptime_seconds gauge
uptime_seconds 3600
"""
            self.wfile.write(metrics.encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        logger.info("%s - %s" % (self.client_address[0], format % args))

def main():
    port = int(os.getenv('METRICS_PORT', 8081))
    logger.info(f"Starting metrics exporter on port {port}")
    server = HTTPServer(('0.0.0.0', port), MetricsHandler)
    server.serve_forever()

if __name__ == '__main__':
    main()
