#!/usr/bin/env bash
set -euo pipefail

URL="${URL:-https://123movienow.cc/wefeed-h5api-bff/subject/play?subjectId=6029989530017612624&se=0&ep=0&detailPath=return-to-silent-hill-UIQMsbNprb7}"
REFERER="${REFERER:-https://123movienow.cc/spa/videoPlayPage/movies/return-to-silent-hill-UIQMsbNprb7?id=6029989530017612624&type=/movie/detail&detailSe=&detailEp=&lang=en}"
COOKIE="${COOKIE:-uuid=202d061a-fbe4-443c-aa09-be0232791ef7}"
X_CLIENT_INFO="${X_CLIENT_INFO:-{\"timezone\":\"Pacific/Pago_Pago\"}}"
DEFAULT_X_USER='{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjc1NjEyMzU2Nzc1NjA4NTMyODAsImF0cCI6MywiZXh0IjoiMTc3MTE0NDQ4MiIsImV4cCI6MTc3ODkyMDQ4MiwiaWF0IjoxNzcxMTQ0MTgyfQ.BF7r1K_npeKWjkjuIxqZMTvUDxJ8oBjvSBc3glWcqtU","userId":"7561235677560853280","userType":0,"appType":3}'
X_USER="${X_USER:-$DEFAULT_X_USER}"
MODE="${MODE:-urls}"

request() {
  curl -sS --compressed "$URL" \
    -H 'accept: application/json' \
    -H 'accept-language: en-US,en;q=0.7' \
    -H 'cache-control: no-cache' \
    -H 'pragma: no-cache' \
    -H 'dnt: 1' \
    -H "referer: $REFERER" \
    -H 'sec-gpc: 1' \
    -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36' \
    -H "x-client-info: $X_CLIENT_INFO" \
    -H "x-user: $X_USER" \
    -H "cookie: $COOKIE"
}

case "$MODE" in
  headers)
    curl -sS --compressed -D - "$URL" \
      -H 'accept: application/json' \
      -H 'accept-language: en-US,en;q=0.7' \
      -H 'cache-control: no-cache' \
      -H 'pragma: no-cache' \
      -H 'dnt: 1' \
      -H "referer: $REFERER" \
      -H 'sec-gpc: 1' \
      -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36' \
      -H "x-client-info: $X_CLIENT_INFO" \
      -H "x-user: $X_USER" \
      -H "cookie: $COOKIE"
    ;;
  json)
    request
    ;;
  urls)
    if command -v jq >/dev/null 2>&1; then
      request | jq -r '
        .data.streams
        | sort_by((.resolutions | tonumber))
        | reverse[]
        | "\(.resolutions)p \(.url)"
      '
    else
      echo "jq is required for MODE=urls. Install jq or use MODE=json." >&2
      exit 1
    fi
    ;;
  *)
    echo "Invalid MODE: $MODE (use: urls, json, headers)" >&2
    exit 1
    ;;
esac

