(function(){
  const els = {
    apiForm: document.getElementById('apiForm'),
    apiBase: document.getElementById('apiBase'),
    mediaType: document.getElementById('mediaType'),
    tmdbId: document.getElementById('tmdbId'),
    season: document.getElementById('season'),
    episode: document.getElementById('episode'),
    fetchStatus: document.getElementById('fetchStatus'),
    jsonForm: document.getElementById('jsonForm'),
    jsonInput: document.getElementById('jsonInput'),
    loadSample: document.getElementById('loadSample'),
    clearJson: document.getElementById('clearJson'),
    streamList: document.getElementById('streamList'),
    streamCount: document.getElementById('streamCount'),
    timingChip: document.getElementById('timingChip'),
    video: document.getElementById('video'),
    nowPlaying: document.getElementById('nowPlaying'),
    pillRow: document.getElementById('pillRow'),
    headerNote: document.getElementById('headerNote'),
    useProxy: document.getElementById('useProxy'),
    proxyWarn: document.getElementById('proxyWarn')
  };

  const samplePayload = {
    success: true,
    tmdbId: '310131',
    imdbId: null,
    count: 3,
    providerTimings: {
      mp4hydra: 404,
      '4khdhub': 2949,
      vidzee: 3022,
      vixsrc: 2914,
      moviesmod: 3230,
      showbox: 7966,
      uhdmovies: 10895
    },
    streams: [
      {
        name: 'VidZee Server5 - 720p - English',
        title: 'VidZee Server5 - 720p - English',
        url: 'NXRRc0JVZmN3VFJsU3FwQmZDbUtBQT09OjQwNWJhUlBSKzZ0c2s1dzBmRGZrRWZtYlYvUkVHVDhZcEdHV2x6UGFhanppTGlQTXR5WUZnN3VSeGk2VE5RNEU=',
        quality: '720p',
        provider: 'VidZee',
        headers: { Referer: 'https://core.vidzee.wtf/' }
      },
      {
        name: 'VidZee Server6 - 720p - Vietnamese',
        title: 'VidZee Server6 - 720p - Vietnamese',
        url: 'UklxTHJKUzhhQzVmT054TkYwcHdyUT09OkYzeEJzVW1lNHpramp3dExaWElHR0FHa1FvbzhmY09tRVZWM2RlTWRYdjNTZmNhZ2RNclpSbnMvZjN3NG1UUDJxRGVObWF5UUNlOUR4ZmEyQU93WEh3PT0=',
        quality: '720p',
        provider: 'VidZee',
        headers: { Referer: 'https://core.vidzee.wtf/' }
      },
      {
        name: 'Vixsrc',
        title: 'Auto Quality Stream',
        url: 'https://vixsrc.to/playlist/232462?token=92601c080033c2c8f1f3cce7552d30f4&expires=1769765524&h=1&lang=en',
        quality: '1080p',
        provider: 'Vidsrc',
        headers: {
          Referer: 'https://vixsrc.to/movie/310131',
          'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'
        }
      }
    ]
  };

  const state = {
    streams: [],
    hls: null,
    providerTimings: null,
    activeIndex: null,
    proxyAvailable: true
  };

  function init() {
    els.headerNote.style.display = 'none';
    els.apiBase.value = `${location.protocol}//${location.host}`;
    els.useProxy.checked = true;
    els.tmdbId.value = '310131';
    els.apiForm.addEventListener('submit', (e) => { e.preventDefault(); fetchFromApi(); });
    els.jsonForm.addEventListener('submit', (e) => { e.preventDefault(); loadFromJson(); });
    els.loadSample.addEventListener('click', () => {
      els.jsonInput.value = JSON.stringify(samplePayload, null, 2);
      loadFromJson();
    });
    els.clearJson.addEventListener('click', () => { els.jsonInput.value = ''; setStatus(''); });
    els.streamList.addEventListener('click', (e) => {
      const card = e.target.closest('.stream-card');
      if (!card) return;
      const idx = Number(card.dataset.idx);
      playStream(idx);
    });
    checkProxyAvailability();
  }

  function setStatus(msg, isError = false) {
    els.fetchStatus.textContent = msg || '';
    els.fetchStatus.classList.toggle('error', !!isError);
  }

  function qualityScore(stream) {
    const q = (stream && stream.quality) || '';
    const match = String(q).match(/(\d{3,4})/);
    return match ? Number(match[1]) : 0;
  }

  async function fetchFromApi() {
    const base = (els.apiBase.value || '').trim().replace(/\/+$/, '');
    const tmdbId = (els.tmdbId.value || '').trim();
    const mediaType = els.mediaType.value;
    const season = (els.season.value || '').trim();
    const episode = (els.episode.value || '').trim();
    if (!base || !tmdbId) {
      setStatus('API base and TMDB ID are required.', true);
      return;
    }
    let url = `${base}/api/streams/${mediaType}/${tmdbId}`;
    const params = new URLSearchParams();
    if (mediaType === 'series') {
      if (season) params.set('season', season);
      if (episode) params.set('episode', episode);
    }
    const query = params.toString();
    if (query) url += `?${query}`;
    setStatus('Fetching streams...');
    try {
      const res = await fetch(url, { credentials: 'include' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      if (!data || !Array.isArray(data.streams)) throw new Error('Invalid response shape.');
      els.jsonInput.value = JSON.stringify(data, null, 2);
      applyTimings(data.providerTimings);
      renderStreams(data.streams);
      setStatus(`Loaded ${data.streams.length} stream(s) from API.`);
    } catch (e) {
      console.error(e);
      setStatus(`Fetch failed: ${e.message}`, true);
    }
  }

  function applyTimings(timings) {
    state.providerTimings = timings || null;
    if (!timings || typeof timings !== 'object') {
      els.timingChip.textContent = '-';
      return;
    }
    const pairs = Object.entries(timings).filter(([, v]) => v !== null);
    if (!pairs.length) { els.timingChip.textContent = '-'; return; }
    const summary = pairs
      .sort((a, b) => a[1] - b[1])
      .map(([name, ms]) => `${name}: ${ms}ms`)
      .join(' - ');
    els.timingChip.textContent = summary;
  }

  function loadFromJson() {
    const raw = els.jsonInput.value.trim();
    if (!raw) { setStatus('Paste JSON first.'); return; }
    try {
      const parsed = JSON.parse(raw);
      if (!parsed || !Array.isArray(parsed.streams)) throw new Error('Missing streams array.');
      applyTimings(parsed.providerTimings);
      renderStreams(parsed.streams);
      setStatus(`Loaded ${parsed.streams.length} stream(s) from JSON.`);
    } catch (e) {
      setStatus(`JSON error: ${e.message}`, true);
    }
  }

  function renderStreams(streams) {
    const cleaned = (streams || []).filter(s => s && s.url);
    cleaned.sort((a, b) => qualityScore(b) - qualityScore(a) || String(a.provider || '').localeCompare(String(b.provider || '')));
    state.streams = cleaned;
    state.activeIndex = null;
    els.streamCount.textContent = `${cleaned.length} available`;
    els.streamList.innerHTML = '';
    if (!cleaned.length) {
      els.streamList.innerHTML = '<p class="muted">No streams found.</p>';
      els.nowPlaying.textContent = 'Nothing loaded';
      els.pillRow.innerHTML = '';
      els.headerNote.style.display = 'none';
      return;
    }
    cleaned.forEach((stream, idx) => {
      const card = document.createElement('button');
      card.type = 'button';
      card.className = 'stream-card';
      card.dataset.idx = idx;
      const needsHeaders = stream.headers && Object.keys(stream.headers).length > 0;
      const encoded = looksEncrypted(stream.url);
      card.innerHTML = `
        <h3>${escapeHtml(stream.name || stream.title || 'Untitled')}</h3>
        <div class="stream-meta">
          <span class="tag">${escapeHtml(stream.provider || 'Unknown')}</span>
          <span class="tag">${escapeHtml(stream.quality || '?')}</span>
          ${needsHeaders ? '<span class="tag">headers</span>' : ''}
          ${encoded ? '<span class="tag">encoded</span>' : ''}
        </div>
        <div class="stream-url">${escapeHtml(truncate(stream.url, 70))}</div>
      `;
      els.streamList.appendChild(card);
    });
    // Auto-play best option
    playStream(0);
  }

  function markActive(idx) {
    state.activeIndex = idx;
    [...els.streamList.querySelectorAll('.stream-card')].forEach((el, i) => {
      el.classList.toggle('active', i === idx);
    });
  }

  async function playStream(idx) {
    const stream = state.streams[idx];
    if (!stream) return;
    markActive(idx);
    setStatus('Resolving stream...');
    try {
      const resolved = await resolveUrl(stream);
      if (!resolved || !resolved.url) throw new Error('No usable URL in stream.');
      loadIntoVideo(resolved, stream);
      updateNowPlaying(stream, resolved.url);
      setStatus(`Playing ${stream.provider || 'source'} (${stream.quality || '?'})`);
    } catch (e) {
      setStatus(e.message, true);
    }
  }

  function updateNowPlaying(stream, url) {
    els.nowPlaying.textContent = `${stream.name || stream.title || 'Stream'} - ${stream.provider || 'Unknown'} ${stream.quality ? `(${stream.quality})` : ''}`;
    const pills = [];
    pills.push(makePill(stream.provider || 'Unknown'));
    if (stream.quality) pills.push(makePill(stream.quality));
    if (looksEncrypted(stream.url)) pills.push(makePill('VidZee token', 'warning'));
    if (stream.headers && Object.keys(stream.headers).length) pills.push(makePill('needs proxy', 'warning'));
    pills.push(makePill(urlLabel(url)));
    els.pillRow.replaceChildren(...pills);
    els.headerNote.style.display = stream.headers && Object.keys(stream.headers).length ? 'block' : 'none';
  }

  function makePill(text, tone) {
    const pill = document.createElement('div');
    pill.className = `pill${tone ? ' ' + tone : ''}`;
    pill.textContent = text;
    return pill;
  }

  function urlLabel(url) {
    if (/\.m3u8(\?|$)/i.test(url)) return 'HLS';
    if (/\.mpd(\?|$)/i.test(url)) return 'DASH';
    if (/\.mp4(\?|$)/i.test(url)) return 'MP4';
    return 'URL ready';
  }

  function destroyHls() {
    if (state.hls) {
      state.hls.destroy();
      state.hls = null;
    }
  }

  function isHlsUrl(url) {
    return /\.m3u8(\?|$)/i.test(url) || url.includes('playlist');
  }

  function loadIntoVideo(resolved, stream) {
    const url = resolved.url;
    state.currentSource = {
      usedProxy: resolved.usedProxy,
      originalUrl: resolved.originalUrl || url,
      raw: resolved.raw || url,
      fallbackTried: resolved.usedProxy ? false : true,
      stream
    };
    destroyHls();
    const isHls = isHlsUrl(url);
    if (isHls && window.Hls && window.Hls.isSupported()) {
      state.hls = new Hls({ lowLatencyMode: true, backBufferLength: 30 });
      state.hls.loadSource(url);
      state.hls.attachMedia(els.video);
      state.hls.on(Hls.Events.ERROR, (_, data) => {
        if (data?.fatal && state.currentSource && state.currentSource.usedProxy && !state.currentSource.fallbackTried) {
          state.currentSource.fallbackTried = true;
          setStatus('Proxy failed; retrying direct URL...');
          destroyHls();
          els.video.src = '';
          loadIntoVideo({ url: state.currentSource.originalUrl, usedProxy: false, originalUrl: state.currentSource.originalUrl, raw: state.currentSource.raw }, stream || state.currentSource.stream);
        }
      });
    } else if (isHls && els.video.canPlayType('application/vnd.apple.mpegurl')) {
      els.video.src = url;
    } else {
      els.video.src = url;
    }
    els.video.play().catch(() => {});
  }

  async function resolveUrl(stream) {
    if (!stream || !stream.url || typeof stream.url !== 'string') return null;
    const raw = stream.url.trim();
    let decoded = raw;
    if (!/^https?:\/\//i.test(raw)) {
      const d = await tryDecodeVidzee(raw);
      decoded = d || raw;
    }
    const useProxy = /^https?:\/\//i.test(decoded) && els.useProxy.checked && state.proxyAvailable && shouldProxy(decoded, stream);
    return {
      url: useProxy ? buildProxyUrl(decoded, stream.headers || {}) : decoded,
      usedProxy: useProxy,
      originalUrl: decoded,
      raw
    };
  }

  function looksEncrypted(str) {
    if (!str || typeof str !== 'string') return false;
    if (/^https?:\/\//i.test(str)) return false;
    try {
      const decoded = atob(normalizeB64(str));
      return decoded.includes(':');
    } catch (e) {
      return false;
    }
  }

  function normalizeB64(str) {
    const s = str.replace(/[^A-Za-z0-9+/=]/g, '');
    const pad = s.length % 4;
    return pad ? s + '='.repeat(4 - pad) : s;
  }

  function b64ToBytes(b64) {
    const bin = atob(normalizeB64(b64));
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  async function tryDecodeVidzee(token) {
    if (!looksEncrypted(token) || !window.crypto?.subtle) return null;
    try {
      const raw = atob(normalizeB64(token));
      const [ivB64, cipherB64] = raw.split(':');
      if (!ivB64 || !cipherB64) return null;
      const iv = b64ToBytes(ivB64.trim());
      const cipherBytes = b64ToBytes(cipherB64.trim());
      const keyBytes = new TextEncoder().encode('qrincywincyspider'.padEnd(32, '\0'));
      const key = await crypto.subtle.importKey('raw', keyBytes, { name: 'AES-CBC' }, false, ['decrypt']);
      const plainBuf = await crypto.subtle.decrypt({ name: 'AES-CBC', iv }, key, cipherBytes);
      const text = new TextDecoder().decode(plainBuf).trim();
      if (/^https?:\/\//i.test(text)) return text;
    } catch (e) {
      console.warn('VidZee decode failed', e);
    }
    return null;
  }

  function normalizedBase() {
    const base = (els.apiBase.value || '').trim();
    if (!base) return `${location.protocol}//${location.host}`;
    return base.replace(/\/+$/, '');
  }

  function shouldProxy(url, stream) {
    const needsHeaders = stream.headers && Object.keys(stream.headers).length > 0;
    const isEncoded = !/^https?:\/\//i.test(url);
    try {
      const u = new URL(url, location.href);
      const sameOrigin = u.origin === location.origin;
      return needsHeaders || !sameOrigin || isEncoded;
    } catch {
      return needsHeaders || isEncoded;
    }
  }

  function buildProxyUrl(targetUrl, headers) {
    if (!state.proxyAvailable) return targetUrl;
    const base = normalizedBase();
    const safeHeaders = headers && Object.keys(headers).length ? headers : {};
    const route = isHls(targetUrl) ? 'm3u8-proxy' : 'ts-proxy';
    const headerPart = Object.keys(safeHeaders).length ? `&headers=${encodeURIComponent(JSON.stringify(safeHeaders))}` : '';
    return `${base}/${route}?url=${encodeURIComponent(targetUrl)}${headerPart}`;
  }

  function isHls(url) {
    return /\.m3u8(\?|$)/i.test(url) || url.includes('playlist');
  }

  async function checkProxyAvailability() {
    try {
      const res = await fetch('/api/config', { credentials: 'include' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      const enabledInConfig = !!(data && data.merged && data.merged.enableProxy);
      state.proxyAvailable = enabledInConfig;
      els.useProxy.disabled = !state.proxyAvailable;
      els.useProxy.checked = state.proxyAvailable;
      if (els.proxyWarn) {
        if (!enabledInConfig) {
          els.proxyWarn.textContent = 'Proxy disabled on the server. Enable it in the admin panel.';
          els.proxyWarn.style.display = 'block';
        } else {
          els.proxyWarn.style.display = 'none';
        }
      }
      if (!state.proxyAvailable) setStatus('Proxy unavailable; using direct URLs. CORS may block some streams.', true);
    } catch (_e) {
      // If config fetch fails, leave defaults as-is
    }
  }

  function truncate(str, max) {
    if (!str) return '';
    if (str.length <= max) return str;
    return `${str.slice(0, max - 3)}...`;
  }

  function escapeHtml(str) {
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  init();
})(); 
