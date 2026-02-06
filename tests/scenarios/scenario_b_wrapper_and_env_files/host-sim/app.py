#!/usr/bin/env python3
"""Mock wrapped application."""
import os, time
from http.server import HTTPServer, BaseHTTPRequestHandler
import logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'OK')

logger.info(f"Starting with config: {os.getenv('APP_CONFIG_PATH')}")
HTTPServer(('0.0.0.0', 8080), Handler).serve_forever()
