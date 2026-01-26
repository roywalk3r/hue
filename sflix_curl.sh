#!/usr/bin/env bash
set -euo pipefail

subject_id="${1:-4024898640031404824}"
stream_id="${2:-6933297213016081600}"
detail_path="${3:-the-flash-qPN2bER4kN4}"
season="${4:-1}"
episode="${5:-1}"

play_url="https://sflix.film/wefeed-h5api-bff/subject/play?subjectId=${subject_id}&se=${season}&ep=${episode}&detailPath=${detail_path}"
caption_url="https://h5-api.aoneroom.com/wefeed-h5api-bff/subject/caption?format=MP4&id=${stream_id}&subjectId=${subject_id}&detailPath=${detail_path}"
referer_url="https://sflix.film/spa/videoPlayPage/movies/${detail_path}?id=${subject_id}&type=/movie/detail&lang=en"

curl -sS "${play_url}" \
  -H 'accept: application/json' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36' \
  -H "referer: ${referer_url}" \
  -H 'cookie: sflix_token=%22eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjMxMTYwODUxODIwODQ5MjQ1MTIsImF0cCI6MywiZXh0IjoiMTc2ODA4NTkyNCIsImV4cCI6MTc3NTg2MTkyNCwiaWF0IjoxNzY4MDg1NjI0fQ.K6fuHWpTcLP0WjQx9ZFe6LSo_oNVNKiuC3MOA2HRw1s%22; account=2904345630854337384|0|H5|1768086238|; uuid=181f2237-fb79-4245-9a8a-2d5f0a60c4aa; sflix_i18n_lang=en'

echo

curl -sS "${caption_url}" \
  -H 'accept: application/json' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36' \
  -H "referer: ${referer_url}"
