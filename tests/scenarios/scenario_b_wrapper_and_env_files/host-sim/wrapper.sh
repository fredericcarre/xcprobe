#!/bin/bash
# Wrapper script that loads env and starts the app
set -e

# Load environment file
if [ -f /etc/wrapped-app/env ]; then
    export $(grep -v '^#' /etc/wrapped-app/env | xargs)
fi

export APP_CONFIG_PATH=/etc/wrapped-app/app.conf
export LOG_PATH=/var/log/apps

cd /opt/wrapped-app
exec su -s /bin/bash appuser -c "python3 /opt/wrapped-app/app.py"
