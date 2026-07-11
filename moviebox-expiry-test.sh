#!/bin/bash
# MovieBox URL expiry probe
# Runs for ~6 hours, probing every 15 minutes

URL="https://bcdnxw.hakunaymatata.com/bt/a37468fd34d6e4b0542691fa0e2a7bf8.mp4?sign=671c5245690000c70257dab7c9e45997&t=1780590542"
LOG="/mnt/devdrive/projects/Misc/hue/moviebox-expiry.log"

echo "[$(date -Iseconds)] Starting MovieBox expiry probe" > "$LOG"
echo "URL: $URL" >> "$LOG"
echo "Probing every 15 min for up to 6 hours" >> "$LOG"
echo "---" >> "$LOG"

for i in $(seq 0 24); do
    ts=$(date -Iseconds)
    elapsed=$((i * 15))
    status=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
        -H "Referer: https://moviebox.pk/" \
        -I "$URL" 2>&1)
    echo "[$ts] +${elapsed}min status=$status" >> "$LOG"
    
    if [ "$status" != "200" ]; then
        echo "[$ts] EXPIRED at +${elapsed} minutes" >> "$LOG"
        break
    fi
    
    sleep 900
done

echo "[$ts] Probe complete" >> "$LOG"
