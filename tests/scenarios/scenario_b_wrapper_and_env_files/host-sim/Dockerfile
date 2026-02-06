# Scenario B: Wrapper script with env files
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    python3 \
    procps \
    iproute2 \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /opt/wrapped-app /etc/wrapped-app /var/log/apps
RUN useradd -r -s /bin/false appuser

COPY wrapper.sh /opt/wrapped-app/
COPY app.py /opt/wrapped-app/
COPY env /etc/wrapped-app/env
COPY app.conf /etc/wrapped-app/app.conf

RUN chmod +x /opt/wrapped-app/wrapper.sh
RUN chown -R appuser:appuser /opt/wrapped-app /var/log/apps

CMD ["/opt/wrapped-app/wrapper.sh"]
