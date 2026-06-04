import urllib.request, re, json, time

# Get fresh API response
req = urllib.request.Request('https://vixsrc.to/api/movie/533535', headers={
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    'Referer': 'https://vixsrc.to/movie/533535'
})
resp = urllib.request.urlopen(req, timeout=5)
data = json.load(resp)
src = data.get('src', '')
print('SRC:', src)

# Immediately hit embed
embed_url = 'https://vixsrc.to' + src if not src.startswith('http') else src
req2 = urllib.request.Request(embed_url, headers={
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    'Referer': 'https://vixsrc.to/movie/533535'
})
resp2 = urllib.request.urlopen(req2, timeout=5)
html = resp2.read().decode()

# Quick scan for any stream URL
for pattern in [r'https?://[^\s\'"]+\.m3u8[^\s\'"]*', r'https?://[^\s\'"]+\.mp4[^\s\'"]*']:
    found = re.findall(pattern, html)
    if found:
        for u in found[:2]:
            print('URL:', u)
        break
