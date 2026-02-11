# Scenario D: Logs to stdout only (12-factor)
FROM node:20-bookworm-slim

RUN apt-get update && apt-get install -y \
    procps \
    iproute2 \
    openssl \
    && rm -rf /var/lib/apt/lists/*

RUN adduser --disabled-password --gecos '' node-app

RUN mkdir -p /app && chown node-app:node-app /app
WORKDIR /app

COPY --chown=node-app:node-app package.json server.js /app/

USER node-app

EXPOSE 3000

CMD ["node", "server.js"]
