# Provider scoring: sticky preferences, popularity, circuit breakers

Three infra modules shape which providers are tried first and which
are skipped outright. They live in `src/infra/`:

- `sticky_pref.rs` — per-user, per-title "which provider worked
  for me last time" cache.
- `provider_score.rs` — per-title global popularity score (drives
  provider ordering within a phase).
- `circuit_breaker.rs` — per-provider failure counter that opens /
  half-opens / closes the circuit.

## Sticky preferences

`StickyPref` is a per-user, per-title cache of the most recent
successful provider. Resolved at the start of a stream fetch
(`routes/streams.rs::streams_handler`):

```rust
struct StickyPrefKey {
    user_id:       String,    // from Bearer token
    media_type:    String,    // "movie" or "tv"
    tmdb_id:       i64,
}
```

If a sticky provider exists and is healthy, it goes first in the
fast phase. The intent: if "I always get Vixsrc working for
Inception on this user", retry that first instead of waiting for
the fan-out to surface it.

Sticky preferences are **read on every fetch but written on every
successful playback report** (`/api/streams/.../report-failure`
is the negative path; successful plays are inferred from the
sticky-pref promotion that happens when `pick_best_stream`
selects a winner).

The cache backend is moka (memory) with a 24-hour TTL.

## Provider popularity

`ProviderScore` is a per-title score driven by global success rate:

```rust
struct ProviderScoreKey {
    media_type:    String,
    tmdb_id:       i64,
    provider:      String,    // real_name, not display name
}
```

The score is the success rate (successes / attempts) over the last
N samples for that title. The dispatch sorts healthy providers
within a phase by score descending — i.e. "providers that
historically work for this title" are tried first.

The cache backend is moka (memory) with a 4-hour TTL. The score
itself decays toward 0 on every read so an old sample doesn't
permanently dominate.

## Circuit breaker

`CircuitBreaker` is a per-provider failure counter. Default config:

- **Failure threshold**: 5 errors → open
- **Open duration**: 30 s
- **Half-open max calls**: 3

State machine:

```
       5 errors
   ┌────────────►
 CLOSED          OPEN
   ▲              │
   │   30 s       │
   │              ▼
   └── 3/3 ok ─── HALF_OPEN
        any fail → re-OPEN
```

While **open**, the dispatcher skips the provider entirely. While
**half-open**, up to 3 calls are allowed through — if all succeed,
the circuit closes; if any fail, it re-opens for another 30 s.

The breaker is per-provider, not per-title. A Vixsrc that
misbehaves for *one* title will still be available for others
unless it exceeds the global failure threshold.

### Where the breaker is wired

`ProviderRegistry::register` wraps each provider in a circuit-
breaker decorator. The decorator's
`fetch` method:

1. Checks the breaker. If open → returns an error without
   calling the underlying provider.
2. Calls the underlying provider. On success → records success
   and returns. On error → records failure and returns the error.

The dispatch counts circuit-open errors toward its own
`min_streams_threshold`, so a temporarily-disabled provider
correctly degrades the fast-phase result count and triggers
the slow phase.

## What the dispatcher actually does with all this

Pseudocode (paraphrased from `providers/dispatch.rs`):

```rust
async fn fetch_from_providers(
    media_type: &str,
    tmdb_id: i64,
    user_id: &str,
) -> Vec<StreamResult> {
    // 1. Sticky-pref lookup
    let sticky = sticky_pref.get(user_id, media_type, tmdb_id);

    // 2. Cache check
    if let Some(cached) = cache.get(...).await { return cached; }

    // 3. Fast phase
    let mut all = vec![];
    let mut stream = stream!();
    for p in healthy_providers().filter(is_fast) {
        if Some(&p.real_name) == sticky.as_ref() { /* priority slot */ }
        let scored = provider_score.get(media_type, tmdb_id, &p.real_name);
        spawn p.fetch().into(&mut stream);
    }
    // ... drain with FAST_PROVIDER_DEADLINE_SECS, NO abort-on-threshold
    all.extend(stream.collect());

    // 4. Slow phase (if thin)
    if all.len() < min_streams_threshold {
        for p in healthy_providers() {
            spawn p.fetch().into(&mut stream);
        }
        // ... drain with SLOW_PROVIDER_DEADLINE_SECS, DO abort on threshold
        all.extend(stream.collect());
    }

    // 5. Cache write
    cache.put(..., &all).await;
    all
}
```

## Tuning knobs

- `MIN_STREAMS_THRESHOLD` — default 3, controls when the slow
  phase fires.
- `FAST_PROVIDER_DEADLINE_SECS` — default 5.
- `SLOW_PROVIDER_DEADLINE_SECS` — default 25.
- `STICKY_PREF_TTL_SECS` — default 24 h.
- `PROVIDER_SCORE_TTL_SECS` — default 4 h.
- `CIRCUIT_BREAKER_FAILURE_THRESHOLD` — default 5.
- `CIRCUIT_BREAKER_OPEN_DURATION_SECS` — default 30.
- `CIRCUIT_BREAKER_HALF_OPEN_MAX_CALLS` — default 3.

None of these are env-tunable today; they are compile-time
constants in `src/infra/*`. If you need to change them in
production, edit and rebuild.
