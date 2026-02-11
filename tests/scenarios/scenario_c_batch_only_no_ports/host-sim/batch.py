#!/usr/bin/env python3
"""Mock batch processor for testing xcprobe."""

import os
import time
import logging

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def main():
    logger.info("Starting batch processor...")
    logger.info(f"BATCH_INPUT_DIR: {os.getenv('BATCH_INPUT_DIR', '/data/input')}")
    logger.info(f"BATCH_OUTPUT_DIR: {os.getenv('BATCH_OUTPUT_DIR', '/data/output')}")
    logger.info(f"DATABASE_URL: {os.getenv('DATABASE_URL', 'not set')}")

    while True:
        logger.info("Processing batch...")
        time.sleep(30)


if __name__ == '__main__':
    main()
