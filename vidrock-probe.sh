#!/bin/bash
LOG="/mnt/devdrive/projects/Misc/hue/vidrock-expiry.log"
LOG2="/mnt/devdrive/projects/Misc/hue/vidrock-nova-expiry.log"

# VidRock Atlas MP4
VIDROCK_URL="https://dreadnought.scp098.workers.dev/https%3A%2F%2Fbcdnxw.hakunaymatata.com%2Ftran-audio%2F20250608%2F19f931175ca6330d4bdfb4ed4a609d43.mp4%3Fsign%3D0c9d2dfe20d4c3dd3d7a5c6745466682%26t%3D1780553063"
# VidRock Nova HLS
NOVA_URL="https://storrrrrrm.site/stream/2aa0e119264f1d5d/master.m3u8"

echo "[$(date -Iseconds)] Starting VidRock expiry probes" > "$LOG"
echo "URL: $VIDROCK_URL" >> "$LOG"
echo "Probing every 15 min" >> "$LOG"
echo "---" >> "$LOG"

echo "[$(date -Iseconds)] Starting VidRock Nova expiry probe" > "$LOG2"
echo "URL: $NOVA_URL" >> "$LOG2"
echo "Probing every 15 min" >> "$LOG2"
echo "---" >> "$LOG2"

for i in $(seq 0 24); do
    ts=$(date -Iseconds)
    elapsed=$((i * 15))

    # Probe Atlas MP4
    status=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
        -H "Referer: https://vidrock.net/" \
        -I "$VIDROCK_URL" 2>/dev/null)
    echo "[$ts] +${elapsed}min atlas_status=$status" >> "$LOG"
    if [ "$status" != "200" ] && [ "$status" != "206" ]; then
        echo "[$ts] Atlas EXPIRED at +${elapsed} minutes" >> "$LOG"
    fi

    # Probe Nova HLS
    status2=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
        -H "Referer: https://vidrock.net/" \
        -I "$NOVA_URL" 2>/dev/null)
    echo "[$ts] +${elapsed}min nova_status=$status2" >> "$LOG2"
    if [ "$status2" != "200" ] && [ "$status2" != "206" ]; then
        echo "[$ts] Nova EXPIRED at +${elapsed} minutes" >> "$LOG2"
    fi

    if [ $i -lt 24 ]; then
        sleep 900
    fi
done

echo "[$(date -Iseconds)] Probes complete" >> "$LOG"
echo "[$(date -Iseconds)] Probes complete" >> "$LOG2"
