# Scenario D: Logs to stdout only (12-factor)
FROM node:20-alpine

RUN adduser -D -s /bin/sh node-app

RUN mkdir -p /app && chown node-app:node-app /app
WORKDIR /app

COPY --chown=node-app:node-app package.json server.js /app/

USER node-app

EXPOSE 3000

CMD ["node", "server.js"]
