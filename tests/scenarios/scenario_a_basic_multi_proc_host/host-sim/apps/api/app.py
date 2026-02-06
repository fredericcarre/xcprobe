#!/usr/bin/env python3
"""Mock API application for testing xcprobe."""

import os
import time
import logging
from flask import Flask, jsonify

# Configure logging
logging.basicConfig(
    level=os.getenv('LOG_LEVEL', 'INFO').upper(),
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

app = Flask(__name__)

# Log environment variables at startup (simulates real app behavior)
logger.info("Starting API server...")
logger.info(f"DATABASE_URL: {os.getenv('DATABASE_URL', 'not set')[:20]}...")
logger.info(f"REDIS_URL: {os.getenv('REDIS_URL', 'not set')}")

@app.route('/health')
def health():
    return jsonify({"status": "ok"})

@app.route('/api/v1/items')
def items():
    return jsonify({"items": []})

@app.route('/')
def index():
    return jsonify({"service": "api", "version": "1.0.0"})

if __name__ == '__main__':
    port = int(os.getenv('API_PORT', 8080))
    logger.info(f"Listening on port {port}")
    app.run(host='0.0.0.0', port=port, debug=False)
