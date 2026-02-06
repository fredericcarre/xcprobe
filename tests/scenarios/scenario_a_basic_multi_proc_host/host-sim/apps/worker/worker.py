#!/usr/bin/env python3
"""Mock worker application for testing xcprobe."""

import os
import time
import logging
import signal
import sys

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

running = True

def signal_handler(signum, frame):
    global running
    logger.info(f"Received signal {signum}, shutting down...")
    running = False

signal.signal(signal.SIGTERM, signal_handler)
signal.signal(signal.SIGINT, signal_handler)

def main():
    logger.info("Starting worker...")
    logger.info(f"DATABASE_URL: {os.getenv('DATABASE_URL', 'not set')[:20]}...")
    logger.info(f"REDIS_URL: {os.getenv('REDIS_URL', 'not set')}")
    logger.info(f"WORKER_CONCURRENCY: {os.getenv('WORKER_CONCURRENCY', '1')}")

    while running:
        logger.debug("Worker processing...")
        time.sleep(5)

    logger.info("Worker shutdown complete")

if __name__ == '__main__':
    main()
