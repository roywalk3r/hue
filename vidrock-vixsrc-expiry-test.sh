#!/bin/bash
# VidRock + Vixsrc URL expiry probe
# Gets fresh URLs then probes every 15 minutes for up to 6 hours

LOG="/mnt/devdrive/projects/Misc/hue/vidrock-vixsrc-expiry.log"
echo "[$(date -Iseconds)] Starting VidRock + Vixsrc expiry probe" > "$LOG"

# --- Get fresh VidRock stream URL ---
# Deadpool (TMDB 533535) - get from API
VIDROCK_URL=$(curl -s "http://localhost:24609/api/streams/movie/533535/live" \
    -H "x-api-key: ${API_KEY:-}" 2>/dev/null | \
    grep -o '"url":"[^"]*"' | head -1 | sed 's/"url":"//;s/"$//')

if [ -z "$VIDROCK_URL" ] || [ "$VIDROCK_URL" = "" ]; then
    echo "[$(date -Iseconds)] WARNING: Could not get VidRock URL, trying TV show fallback" >> "$LOG"
    VIDROCK_URL=$(curl -s "http://localhost:24609/api/streams/tv/1402/live?season=1&episode=1" \
        -H "x-api-key: ${API_KEY:-}" 2>/dev/null | \
        grep -o '"url":"[^"]*"' | head -1 | sed 's/"url":"//;s/"$//')
fi

# --- Get fresh Vixsrc stream URL ---
VIXSRC_URL=$(curl -s "http://localhost:24609/api/streams/movie/533535/live" \
    -H "x-api-key: ${API_KEY:-}" 2>/dev/null | \
    grep -o '"url":"[^"]*"' | sed 's/"url":"//;s/"$//' | grep "vixsrc" | head -1)

if [ -z "$VIXSRC_URL" ] || [ "$VIXSRC_URL" = "" ]; then
    echo "[$(date -Iseconds)] WARNING: Could not get Vixsrc URL from live endpoint, trying direct" >> "$LOG"
    VIXSRC_URL=$(curl -s "http://localhost:24609/api/streams/movie/533535" \
        -H "x-api-key: ${API_KEY:-}" 2>/dev/null | \
        grep -o '"url":"[^"]*"' | sed 's/"url":"//;s/"$//' | grep "vixsrc" | head -1)
fi

echo "VidRock URL: $VIDROCK_URL" >> "$LOG"
echo "Vixsrc URL: $VIXSRC_URL" >> "$LOG"
echo "Probing every 15 min for up to 6 hours" >> "$LOG"
echo "---" >> "$LOG"

for i in $(seq 0 24); do
    ts=$(date -Iseconds)
    elapsed=$((i * 15))

    if [ -n "$VIDROCK_URL" ] && [ "$VIDROCK_URL" != "" ]; then
        vidrock_status=$(curl -s -o /dev/null -w "%{http_code}" \
            -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
            -H "Referer: https://vidrock.net/" \
            -H "Origin: https://vidrock.net" \
            -I "$VIDROCK_URL" 2>/dev/null)
        echo "[$ts] +${elapsed}min vidrock_status=$vidrock_status" >> "$LOG"
        if [ "$vidrock_status" != "200" ] && [ "$vidrock_status" != "206" ]; then
            echo "[$ts] VidRock EXPIRED at +${elapsed} minutes (status=$vidrock_status)" >> "$LOG"
        fi
    fi

    if [ -n "$VIXSRC_URL" ] && [ "$VIXSRC_URL" != "" ]; then
        vixsrc_status=$(curl -s -o /dev/null -w "%{http_code}" \
            -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
            -H "Referer: https://vixsrc.to/" \
            -I "$VIXSRC_URL" 2>/dev/null)
        echo "[$ts] +${elapsed}min vixsrc_status=$vixsrc_status" >> "$LOG"
        if [ "$vixsrc_status" != "200" ] && [ "$vixsrc_status" != "206" ]; then
            echo "[$ts] Vixsrc EXPIRED at +${elapsed} minutes (status=$vixsrc_status)" >> "$LOG"
        fi
    fi

    sleep 900
done

echo "[$(date -Iseconds)] Probe complete" >> "$LOG"
