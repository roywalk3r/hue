#!/bin/bash
# Capture a fresh Vixsrc stream URL and probe it

LOG="/mnt/devdrive/projects/Misc/hue/vixsrc-expiry.log"
echo "[$(date -Iseconds)] Starting Vixsrc stream URL expiry probe" > "$LOG"

# Try to capture a fresh stream URL from the Hue API
VIXSRC_URL=""
for attempt in $(seq 1 10); do
    VIXSRC_URL=$(curl -s "http://localhost:24609/api/streams/movie/533535" \
        -H "x-api-key: ${API_KEY:-}" 2>/dev/null | \
        python3 -c "
import json, sys, re
try:
    data = json.load(sys.stdin)
    for s in data.get('streams', []):
        if 'aura' in s.get('provider','').lower() or 'vixsrc' in s.get('url','').lower():
            print(s.get('url',''))
            break
except:
    pass
" 2>/dev/null)
    if [ -n "$VIXSRC_URL" ] && [ "$VIXSRC_URL" != "" ]; then
        break
    fi
    sleep 1
done

if [ -z "$VIXSRC_URL" ] || [ "$VIXSRC_URL" = "" ]; then
    echo "[$(date -Iseconds)] Could not capture Vixsrc URL from API" >> "$LOG"
    exit 1
fi

echo "URL: $VIXSRC_URL" >> "$LOG"
echo "Probing every 1 min for up to 30 min (Vixsrc expires fast)" >> "$LOG"
echo "---" >> "$LOG"

for i in $(seq 0 30); do
    ts=$(date -Iseconds)
    elapsed=$((i))
    status=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
        -H "Referer: https://vixsrc.to/" \
        -I "$VIXSRC_URL" 2>/dev/null)
    echo "[$ts] +${elapsed}min status=$status" >> "$LOG"

    if [ "$status" != "200" ] && [ "$status" != "206" ]; then
        echo "[$ts] Vixsrc EXPIRED at +${elapsed} minutes" >> "$LOG"
        break
    fi

    if [ $i -lt 30 ]; then
        sleep 60
    fi
done

echo "[$(date -Iseconds)] Probe complete" >> "$LOG"
