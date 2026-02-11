# Host simulator - runs multiple processes via supervisor
FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    python3 \
    python3-pip \
    supervisor \
    procps \
    iproute2 \
    netcat-openbsd \
    curl \
    openssl \
    && rm -rf /var/lib/apt/lists/*

# Install Python packages for mock apps
RUN pip3 install --break-system-packages flask gunicorn redis psycopg2-binary

# Create app directories
RUN mkdir -p /opt/api /opt/worker /etc/supervisor/conf.d /var/log/apps

# Copy app configurations
COPY supervisord.conf /etc/supervisor/supervisord.conf
COPY apps/ /opt/

# Copy environment files
COPY env/api.env /etc/default/api
COPY env/worker.env /etc/default/worker

# Create app users
RUN useradd -r -s /bin/false api-user && \
    useradd -r -s /bin/false worker-user

# Set permissions
RUN chown -R api-user:api-user /opt/api && \
    chown -R worker-user:worker-user /opt/worker

# Expose ports
EXPOSE 8080 8081

# Start supervisor
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/supervisord.conf"]
