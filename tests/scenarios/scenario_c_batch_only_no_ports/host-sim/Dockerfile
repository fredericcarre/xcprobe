# Scenario C: Batch job with no ports
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    python3 \
    procps \
    iproute2 \
    openssl \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /opt/batch /etc/batch /var/log/batch
RUN useradd -r -s /bin/false batch-user

COPY batch.py /opt/batch/batch.py
COPY config.yaml /etc/batch/config.yaml

RUN chown -R batch-user:batch-user /opt/batch /var/log/batch

CMD ["su", "-s", "/bin/bash", "batch-user", "-c", "python3 /opt/batch/batch.py"]
