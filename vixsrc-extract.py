import urllib.request, urllib.parse, re, json, time

# Step 1: Get fresh API response
api_url = 'https://vixsrc.to/api/movie/533535'
req = urllib.request.Request(api_url, headers={
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    'Referer': 'https://vixsrc.to/movie/533535'
})
resp = urllib.request.urlopen(req, timeout=10)
data = json.load(resp)
src = data.get('src', '')
embed_url = 'https://vixsrc.to' + src if not src.startswith('http') else src
print('EMBED_URL:', embed_url)

m = re.search(r'expires=(\d+)', src)
if m:
    expires_ts = int(m.group(1))
    now_ts = int(time.time())
    print(f'EXPIRES: {expires_ts} (in {expires_ts - now_ts}s)')

# Step 2: Immediately hit embed page
req2 = urllib.request.Request(embed_url, headers={
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    'Referer': 'https://vixsrc.to/movie/533535'
})
resp2 = urllib.request.urlopen(req2, timeout=10)
html = resp2.read().decode()

# Extract stream URLs
m3u8 = re.findall(r'https?://[^\s\'"<>]+\.m3u8[^\s\'"<>]*', html)
mp4 = re.findall(r'https?://[^\s\'"<>]+\.mp4[^\s\'"<>]*', html)
for u in m3u8[:3]:
    print('STREAM:', u)
for u in mp4[:3]:
    print('STREAM:', u)

# Look for masterPlaylist
m = re.search(r'window\.masterPlaylist\s*=\s*(\{[^\}]+\})', html)
if m:
    block = m.group(1)
    url = re.search(r'url\s*:\s*["\']([^"\']+)["\']', block)
    token = re.search(r'token\s*:\s*["\']([^"\']+)["\']', block)
    expires = re.search(r'expires\s*:\s*["\']([^"\']+)["\']', block)
    if url:
        u = url.group(1)
        if token and expires:
            sep = '&' if '?' in u else '?'
            u = f"{u}{sep}token={token.group(1)}&expires={expires.group(1)}&h=1&lang=en"
        print('MASTER:', u)
