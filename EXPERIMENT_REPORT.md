# Browser-Native Data Science Ecosystem Experiment Report

Date: 2026-03-16  
Project: `datascienceecosystem`

## Scope

This report summarizes the browser experiments implemented in this repository, the manual run results observed in Chromium, and the conclusions supported by those runs.

The intent is to let a reader:

1. understand what was tested,
2. see the concrete measured results,
3. inspect the exact HTML / Rust / Wasm prototypes used,
4. reproduce the runs locally.

---

## Reproduction basics

### Server

Run the local static server with COOP/COEP headers:

```bash
node serve.mjs
```

Default URL:

```text
http://127.0.0.1:8787/
```

The server file is:

- `serve.mjs`

It sets:

- `Cross-Origin-Opener-Policy: same-origin`
- `Cross-Origin-Embedder-Policy: require-corp`
- `Cross-Origin-Resource-Policy: same-origin`

Those headers are required for `SharedArrayBuffer` and shared `WebAssembly.Memory`.

---

## Data used

### Synthetic Parquet test data

Generated with:

- `scripts/generate_synthetic_parquet.py`

Primary large test file:

- `data/synthetic-6gib.parquet`

Observed generation result:

- size: `6.02 GiB`
- bytes: `6,468,289,241`
- rows: `38,500,000`
- row groups: `154`

Schema:

- `id` `int64`
- `partition` `int32`
- `feature_a` `double`
- `feature_b` `double`
- `feature_c` `double`
- `label` `int8`
- `payload` `fixed_size_binary(128)`

Supporting small file:

- `data/test-small.parquet`

---

## Result summary

| Experiment | File | Result |
|---|---|---|
| MVT 1 SAB zero-copy workers | `index.html` | PASS |
| Shared Wasm memory workers | `wasm-workers.html` | PASS at 1 GiB shared memory; single 8 GiB shared memory not yet established |
| Greedy memory bank probe | `memory-bank.html` | PASS at 6 GiB retained bank |
| MVT 2 persistence / anti-throttling | `mvt2-persistence.html` | PASS |
| Parquet -> DuckDB -> Arrow -> shard bank routing | `parquet-arrow-bank.html` | PASS |
| Distributed Arrow analytics over shard bank | `parquet-arrow-analytics.html` | PASS |
| Zero-copy Rust byte reads from shard bank | `rust-zero-copy-shardbank.html` | PASS |
| Zero-copy Rust typed aggregates over shard bank | `rust-typed-analytics.html` | PASS |
| Rust Arrow IPC parsing over shard bank | `rust-arrow-ipc-analytics.html` | NOT VALIDATED / TOO SLOW |
| Rust Arrow IPC diagnostic | `rust-arrow-ipc-diagnostic.html` | FAIL (`unreachable`) |
| Polars browser feasibility | `polars-spike.html` | PASS |
| Polars typed-column bridge | `polars-typed-bridge.html` | PASS |
| Polars shard-bank bridge | `polars-shardbank-bridge.html` | PASS |
| Polars zero-copy ownership micro-proof | `polars-zero-copy-microproof.html` | PASS at 10M and 100M rows; 1B-row single-arena attempt failed |
| Monolithic memory proof | `monolithic-memory-proof.html` | PASS as architectural proof: single shared Wasm memory works to 4 GiB and fails above that, so the compute substrate must be sharded for 10 GiB+ targets |
| Polars bank-chunked shim | `polars-bank-chunked-shim.html` | PASS for correctness; slower than contiguous arena path in single-worker mode |
| Polars bank-chunked workers | `polars-bank-chunked-workers.html` | PASS for correctness; chunked shim remains slower than arena, but is still viable under equal worker parallelism |
| Polars Arrow-view live workers | `polars-arrow-view-live-workers.html` | PASS for real shared-bank manifest consumption and fresh-worker mutation tracking through both live-view and Polars paths |
| Parquet -> bank -> Polars live workers | `parquet-polars-bank-live-workers.html` | PASS as the strongest practical end-to-end proof: real Parquet scan, routed bank, fresh live workers, fresh Polars workers, exact mutation tracking |

---

# 1. MVT 1 — SAB zero-copy memory proof

## Prototype

- `index.html`

## Goal

Show that multiple Web Workers can access the same large `SharedArrayBuffer` without memory scaling by worker count.

## Manual validation used

The key manual validation compared a `1.00 GiB` shared allocation with:

- `1 worker`
- `4 workers`

Observed Chromium Task Manager memory footprint:

- `1 worker`: approximately `1,078,956K` to `1,081,768K`
- `4 workers`: approximately `1,095,572K`

## Conclusion

This is a clean MVT 1 pass.

Why:

- the tab footprint stayed close to the original allocation size,
- memory did **not** scale toward `~4x` for `4` workers,
- computation results matched expected sums.

## Notes

Earlier larger runs also succeeded functionally, including a `~1.98 GiB` allocation, but the `1 worker` vs `4 workers` comparison is the strongest zero-copy proof.

---

# 2. Shared Wasm memory workers

## Prototype

- `wasm-workers.html`
- `wasm/sum_range.rs`
- `wasm/sum_range.wasm`

## Goal

Validate that the shared-memory worker pattern can be carried into Wasm-backed workers using imported shared `WebAssembly.Memory`.

## Result

At `1 GiB` shared Wasm memory, `4` workers, the test ran successfully and verified the expected sums.

## Important limit identified

A single huge shared Wasm memory is not something we can assume will scale arbitrarily. The architecture should assume sharding rather than one monolithic shared Wasm memory.

## Conclusion

Shared Wasm memory workers are viable, but the practical path remains a sharded bank.

---

# 3. Greedy sharded memory bank

## Prototype

- `memory-bank.html`

## Goal

Probe how much shared memory a Chromium tab can hold by retaining a bank of shared shards.

## Manual run used

Configuration:

- shard size: `256 MiB`
- target total: `6 GiB`

Observed result:

- allocated shards: `24`
- total allocated: `6.00 GiB`
- recommended usable: `5.10 GiB`
- Chromium Task Manager memory footprint: approximately `6,322,820K`

Observed browser/system characteristics during this run:

- `navigator.deviceMemory` reported `8 GiB`
- system RAM visible in host monitor was `31.1 GiB`
- this confirms the expected privacy-capped `deviceMemory` behavior

## Conclusion

This is a pass.

A single browser tab can retain a multi-gigabyte sharded memory bank, and `6 GiB` was demonstrated explicitly.

---

# 4. MVT 2 — persistence / anti-throttling

## Prototype

- `mvt2-persistence.html`

## Goal

Show that a long-running browser workload with memory + worker activity can survive minimization / backgrounding when anchored with Document Picture-in-Picture and Wake Lock.

## Manual run used

Configuration:

- shared memory bank: `6.00 GiB`
- workers: `4`
- duration: `10 minutes`
- heartbeat interval: `1 second`

Observed result:

- elapsed: `600.0 s`
- expected heartbeats per worker: `600`
- actual heartbeats per worker:
  - worker 0: `600`
  - worker 1: `600`
  - worker 2: `600`
  - worker 3: `600`
- worst heartbeat gap: `1005 ms`
- final UI result: `SUCCESS`

Per-worker summary observed:

- worker 0 max gap: `1001 ms`
- worker 1 max gap: `1002 ms`
- worker 2 max gap: `1005 ms`
- worker 3 max gap: `1002 ms`

## Interpretation

This is a pass.

A `1 s` heartbeat with worst gap around `1005 ms` indicates effectively no meaningful background throttling during the 10-minute run.

## Note

A run displayed `Wake lock was released.` in the UI, but the heartbeat record still remained clean. So the practical persistence result passed, even though wake-lock lifecycle handling could still be made more robust.

---

# 5. Parquet -> DuckDB-Wasm -> Arrow -> shard bank

## Prototype

- `parquet-arrow-bank.html`

## Goal

Validate the structural data path:

1. pick local Parquet,
2. register it in DuckDB-Wasm,
3. query it as Arrow batches,
4. route those batches into the sharded memory bank,
5. reconstruct a stored batch in a worker.

## Manual run used

Query:

```sql
SELECT * FROM parquet_scan('selected.parquet') LIMIT 100000;
```

Observed routed result with `max batches = 32`:

- batches: `32`
- batch size from DuckDB stream: `2048 rows`
- rows routed: `65,536`
- total Arrow bytes routed: `66.7 MiB`
- per batch IPC size: approximately `2.08 MiB`
- placements per batch: `1`

Worker verification result:

- reconstructed batch 0 successfully
- rows recovered: `2048`

## Caveat

The `payload` field type fidelity was not perfect through the JS reconstruction path. The routing proof passed, but exact schema fidelity for complex/binary types is not yet the strongest part of this prototype.

## Conclusion

Pass for routing/manifests/worker reconstruction.

---

# 6. Distributed Arrow analytics over shard bank

## Prototype

- `parquet-arrow-analytics.html`

## Goal

Run real distributed analytics over routed Arrow data stored in the sharded bank and compare the result to DuckDB.

## Manual run used

Query:

```sql
SELECT partition, feature_a, feature_b, feature_c, label FROM parquet_scan('selected.parquet');
```

Configuration:

- memory bank: `12 GiB`
- workers: `4`

Observed routing result:

- rows routed: `38,500,000`
- batches routed: `18,799`
- Arrow bytes routed: `1.05 GiB`
- routing time: `11.9 s`

Observed worker aggregate:

- `rowCount`: `38,500,000`
- `sumFeatureA`: `4698.863799800984`
- `sumFeatureB`: `385030385.02823937`
- `sumFeatureC`: `-1809968.4938077368`
- `label0`: `19,248,502`
- `label1`: `19,251,498`

Observed DuckDB reference aggregate:

- `rowCount`: `38,500,000`
- `sumFeatureA`: `4698.863799801584`
- `sumFeatureB`: `385030385.0282272`
- `sumFeatureC`: `-1809968.4938078541`
- `label0`: `19,248,502`
- `label1`: `19,251,498`

## Interpretation

This is a pass.

- row counts matched exactly
- label counts matched exactly
- float sums differed only by tiny floating-point accumulation noise

---

# 7. Zero-copy Rust reads from shared Wasm-memory shard bank

## Prototype

- `rust-zero-copy-shardbank.html`
- `wasm/checksum_range.rs`
- `wasm/checksum_range.wasm`

## Goal

Replace plain SAB shards with shared `WebAssembly.Memory` shards and prove that Rust/Wasm can read routed bytes directly from those shared memories with no copy into a separate Rust-owned buffer.

## Manual run used

Configuration:

- bank size: `12.0 GiB`
- shards: `48`
- workers: `4`
- rows routed: `38,500,000`
- routed Arrow bytes: `1,129,734,496` bytes (`1.05 GiB`)
- routing time: `15.8 s`
- proof time: `0.2 s`

Observed checksums:

- JS checksum: `120,496,068,369`
- Rust checksum: `120,496,068,369`

## Conclusion

Pass.

This is the clean zero-copy Rust memory proof over a shared Wasm-memory shard bank.

---

# 8. Real typed Rust aggregates over zero-copy shard bank

## Prototype

- `rust-typed-analytics.html`
- `wasm/typed_aggregates.rs`
- `wasm/typed_aggregates.wasm`

## Goal

Move beyond checksums and show real typed computation in Rust directly over the bank.

## Manual run used

Query:

```sql
SELECT feature_a, feature_b, feature_c, label FROM parquet_scan('selected.parquet');
```

Observed routing result:

- rows routed: `38,500,000`
- batches routed: `18,799`
- typed bytes routed: approximately `1.00 GiB`
- routing time: `7.7 s`

Observed Rust aggregate:

- `rowCount`: `38,500,000`
- `sumFeatureA`: `4698.863799801158`
- `sumFeatureB`: `385030385.02822447`
- `sumFeatureC`: `-1809968.4938072574`
- `label0`: `19,248,502`
- `label1`: `19,251,498`

Observed DuckDB reference:

- `rowCount`: `38,500,000`
- `sumFeatureA`: `4698.863799801584`
- `sumFeatureB`: `385030385.0282272`
- `sumFeatureC`: `-1809968.4938078541`
- `label0`: `19,248,502`
- `label1`: `19,251,498`

Observed proof time:

- `0.1 s`

## Conclusion

Pass.

This is the strongest successful low-level compute result in the repo:

- real Rust aggregates,
- zero-copy reads from the bank,
- correctness against DuckDB,
- real dataset size.

---

# 9. Rust Arrow IPC parsing over shard bank

## Prototypes

- `rust-arrow-ipc-analytics.html`
- `rust-arrow-ipc-diagnostic.html`
- `rust-arrow-ipc/Cargo.toml`
- `rust-arrow-ipc/src/lib.rs`
- `wasm/rust_arrow_ipc.wasm`

## Goal

Try to move one layer higher and let Rust parse Arrow IPC batches directly from shared shard-bank memory using Rust Arrow IPC crates.

## Full proof attempt result

The full proof attempt over many routed batches remained CPU-bound for a long time and was not practical to validate as a clean pass.

## Diagnostic result

The smaller diagnostic used:

- routed batches: `10`
- routed rows: `20,480`
- routed Arrow bytes: `506 KiB`

Observed result:

- immediate failure with status text / log showing: `unreachable`

## Conclusion

This path is **not validated**.

Current evidence indicates that Rust Arrow IPC parsing in this Wasm/shared-memory setup is not yet a reliable building block for this project.

---

# 10. Polars browser feasibility spike

## Prototype

- `polars-spike.html`
- `polars-spike/Cargo.toml`
- `polars-spike/src/lib.rs`
- `polars-web/polars_spike.js`
- `polars-web/polars_spike_bg.wasm`

## Goal

Check whether Polars can compile to browser Wasm and execute DataFrame-like workloads in Chromium.

## Observed result

Wasm module:

- size: `10.9 MiB`

Load/init time observed in one run:

- `52.9 ms`

Smoke test result:

- `5.5`

Generated workload results observed:

- `1,000,000` rows -> `41.1 ms`
- `5,000,000` rows -> `152.1 ms`
- `10,000,000` rows -> `273.2 ms`
- `100,000,000` rows -> `2890.9 ms`

At `100,000,000` rows:

- Polars sum matched JS exactly: `6,975,000,134`
- Polars label1 count matched JS exactly: `46,666,667`

## Conclusion

Pass.

Polars is feasible in the browser and remains performant even at high synthetic row counts.

---

# 11. Polars typed-column bridge spike

## Prototype

- `polars-typed-bridge.html`
- same Polars wasm assets as above

## Goal

Check whether browser Polars can consume realistic typed columns and compute correct aggregates.

## Manual run used

Observed run:

- rows: `10,000,000`
- load/init: `51.8 ms`
- bridge test time: `295.1 ms`

Results:

- Polars sum: `697,500,134`
- JS sum: `697,500,134`
- Polars label1 count: `4,666,667`
- JS label1 count: `4,666,667`
- partition 0 count: `1,250,000`
- partition 1 count: `1,250,000`

## Conclusion

Pass.

Polars is viable as a typed-column consumer.

Important caveat:

- this is still a copy-based bridge into Polars-owned memory,
- not zero-copy integration with the shard bank.

---

# 12. Polars shard-bank bridge spike

## Prototype

- `polars-shardbank-bridge.html`

## Goal

Measure the actual cost of bridging from realistic typed data stored in a shared shard bank into Polars-owned memory.

## Manual run A — 10,000,000 rows

Configuration:

- rows: `10,000,000`
- shard size: `64 MiB`
- target bank: `1024 MiB`
- actual bank size: `1.00 GiB`

Observed result:

- routed typed bytes: `160,000,000`
- fill time: `154.8 ms`
- bridge copy time: `59.0 ms`
- Polars compute time: `293.5 ms`
- total time: `509.8 ms`
- correctness: pass

## Manual run B — 100,000,000 rows

Configuration:

- rows: `100,000,000`
- shard size: `1024 MiB`
- target bank: `10240 MiB`
- actual bank size: `10.0 GiB`

Observed result:

- routed typed bytes: `1,600,000,000`
- fill time: `1386.4 ms`
- bridge copy time: `528.5 ms`
- Polars compute time: `3098.0 ms`
- total time: `5014.2 ms`
- correctness: pass

## Interpretation

This is a strong pass for a copy bridge.

At `100M` rows:

- the bridge copy is significant,
- but it is still much smaller than Polars compute,
- so Polars remains plausible as a higher-level layer on top of bank data.

---

# 13. Polars zero-copy ownership micro-proof

## Prototype

- `polars-zero-copy-microproof.html`
- `polars-spike/src/lib.rs`
- `polars-web/polars_spike.js`
- `polars-web/polars_spike_bg.wasm`

## Goal

Test whether Polars performs materially better when the browser writes data directly into Polars-owned Wasm buffers and Polars then takes ownership of those buffers without an additional ingest copy.

## Important scope caveat

This is **not** shard-bank integration.

It is a narrower micro-proof for a **Polars-friendly arena layout**:

- JS allocates typed buffers from Rust/Wasm,
- JS fills those buffers directly in Wasm memory,
- Rust/Polars consumes them with ownership transfer,
- the experiment checks whether that ownership-friendly ingestion path is faster than a more normal Polars-owned generation path.

So the question answered here is:

> if data is already arranged the way Polars wants, can we avoid a meaningful amount of ingest overhead?

## Rust/Wasm additions used

The Polars Wasm module was extended with the following exported functions:

- `polars_alloc_f64_buffer(rows)`
- `polars_alloc_i32_buffer(rows)`
- `polars_from_owned_buffers(feature_a_ptr, label_ptr, partition_ptr, rows)`

The key implementation detail is that `polars_from_owned_buffers(...)` adopts already-populated Wasm buffers using `Vec::from_raw_parts(...)`, so the zero-copy proof is about **ownership transfer of already-filled buffers**, not copying browser arrays into new Polars vectors.

## Manual run A — 10,000,000 rows

Observed result:

- copy-path Polars time: `378.8 ms`
- zero-copy fill into Polars-owned Wasm memory: `44.9 ms`
- zero-copy Polars compute time: `188.8 ms`
- copy sum: `697,500,134`
- zero-copy sum: `697,500,134`
- JS reference sum: `697,500,134`
- copy label1 count: `4,666,667`
- zero-copy label1 count: `4,666,667`
- correctness: pass

## Manual run B — 100,000,000 rows

Observed result:

- copy-path Polars time: `2797.2 ms`
- zero-copy fill into Polars-owned Wasm memory: `397.5 ms`
- zero-copy Polars compute time: `1785.2 ms`
- copy sum: `6,975,000,134`
- zero-copy sum: `6,975,000,134`
- JS reference sum: `6,975,000,134`
- copy label1 count: `46,666,667`
- zero-copy label1 count: `46,666,667`
- correctness: pass

## Manual run C — 1,000,000,000 rows

Observed result:

- status: failure with `unreachable`

## Interpretation

This experiment is one of the clearest architectural signals in the repo.

At `100M` rows:

- baseline Polars path: `2797.2 ms`
- zero-copy Polars compute: `1785.2 ms`

That is a reduction of roughly `36%` in the Polars-side compute/ingest stage.

Even if the buffer fill time is included:

- zero-copy total relevant work: approximately `397.5 + 1785.2 = 2182.7 ms`
- baseline Polars path: `2797.2 ms`

So the ownership-friendly path still wins overall in this micro-proof.

## What this shows

1. **Polars benefits materially from a Polars-friendly arena.**
   - If Polars can adopt already-populated buffers, it performs significantly better.

2. **The right design target is not “make the shard bank itself be Polars.”**
   - The better model is: shard bank -> contiguous partition arena -> Polars.

3. **You do not need one physical shard per column.**
   - But you likely do want contiguous typed buffers per column chunk / partition when preparing data for Polars.

4. **The `1B` failure does not invalidate the result.**
   - With three columns here, raw data alone is about `16 GiB` (`8 GiB` for `f64` + `4 GiB` + `4 GiB` for two `i32` columns) before allocator overhead and browser/Wasm limits.
   - The failure is consistent with practical single-arena memory limits in the browser, not with a logic failure of the approach.

## Conclusion

This is a strong pass for the idea that:

- Polars belongs on top of a Polars-friendly ownership / arena layer,
- not directly on top of an arbitrary fragmented storage substrate,
- and not necessarily with one shard per column,
- but with contiguous column buffers per partition when handing data to Polars.

---

# 14. Monolithic memory proof

## Prototype

- `monolithic-memory-proof.html`
- `wasm/checksum_range.rs`
- `wasm/checksum_range.wasm`

## Goal

Test whether a non-sharded design is actually viable by separating two questions:

1. can Chromium retain and share one giant monolithic `SharedArrayBuffer`?
2. can Chromium retain and share one giant monolithic shared `WebAssembly.Memory` suitable for Rust/Wasm compute?

The second question is the architecturally important one for this project.

## Manual run used

Configuration:

- worker count: `4`
- touch/checksum stride: `4096` bytes
- architecture target: `10 GiB`
- sweep sizes (MiB): `256, 512, 1024, 2048, 4096, 6144, 8192, 10240, 12288`

## Monolithic SAB result

Observed successful sizes:

- `256 MiB`
- `512 MiB`
- `1 GiB`

Observed failure:

- `2 GiB` -> `Array buffer allocation failed`

## Monolithic shared WebAssembly.Memory result

Observed successful sizes:

- `256 MiB`
- `512 MiB`
- `1 GiB`
- `2 GiB`
- `4 GiB`

Observed failure:

- `6 GiB` -> `WebAssembly.Memory(): Property 'initial': value 98304 is above the upper bound 65536`

## Interpretation

This is a strong architectural proof.

The key result is not the monolithic SAB ceiling, which may vary by machine and browser configuration.
The key result is the monolithic shared `WebAssembly.Memory` ceiling.

The failure at `6 GiB` is explained directly by the Wasm page upper bound observed in this environment:

- `65536` Wasm pages maximum
- `65536 * 64 KiB = 4 GiB`

So in the tested Chromium / wasm32 environment:

- one monolithic shared Wasm linear memory is viable up to `4 GiB`
- one monolithic shared Wasm linear memory is **not** viable above `4 GiB`

## Conclusion

For the intended architecture:

- Rust/Wasm compute
- browser Polars / Wasm-friendly analytics
- multi-worker shared-memory execution
- `10 GiB+` target datasets

A single monolithic shared Wasm memory is not enough.

Therefore:

- the compute-visible browser memory substrate must be **sharded**
- sharding is not merely an optimization here
- it is an architectural requirement for the target size class

---

# 15. Polars bank-chunked shim

## Prototype

- `polars-bank-chunked-shim.html`
- `polars-spike/src/lib.rs`
- `polars-web/polars_spike.js`
- `polars-web/polars_spike_bg.wasm`

## Goal

Test the earlier-shim idea directly:

- keep data in a bank-like segmented layout,
- expose each segment as a chunked typed column to Polars,
- and compare that against both:
  - a generated Polars-owned baseline,
  - and the contiguous ownership-transfer arena path.

This experiment is still synthetic typed-bank data, not real DuckDB-routed shard-bank data yet. Its purpose is to answer whether Polars can consume segmented chunked columns early enough that the custom layer stays thin.

## Manual run A — 10,000,000 rows

Configuration:

- rows: `10,000,000`
- segment rows: `1,000,000`
- segment count: `10`

Observed result:

- generated baseline: `371.3 ms`
- arena path total: `237.8 ms`
- chunked shim total: `558.5 ms`
- correctness: pass

## Manual run B — 100,000,000 rows

Configuration:

- rows: `100,000,000`
- segment rows: `5,000,000`
- segment count: `20`

Observed result:

- generated baseline Polars: `2278.2 ms`
- arena path fill: `952.6 ms`
- arena path Polars: `2014.8 ms`
- arena path total: `2967.4 ms`
- chunked shim fill: `701.3 ms`
- chunked shim Polars: `7988.0 ms`
- chunked shim total: `8689.3 ms`
- correctness: pass

## Repeat observation at 100,000,000 rows

A nearby repeat run also showed the same qualitative result:

- generated baseline: `2973.0 ms`
- arena path total: `2419.1 ms`
- chunked shim total: `8080.0 ms`
- correctness: pass

So although the exact timings moved between runs, the conclusion remained stable: the chunked shim stayed much slower than the arena path.

## Interpretation

This experiment is valuable because it answers the earlier-shim question directly.

The result is:

- **correctness:** yes
- **thin chunked shim into Polars:** yes
- **competitive performance versus arena path:** no

At `100M` rows, the biggest difference is in the Polars-side stage:

- arena Polars: about `2014.8 ms`
- chunked-shim Polars: about `7988.0 ms`

That is roughly a `4x` slowdown on the Polars stage for the segmented chunked path.

So while Polars can consume segmented chunked columns correctly, it does not like that layout nearly as much as contiguous ownership-friendly arenas.

## Conclusion

This is a pass for correctness, but in single-worker form it did not support replacing the arena path.

The practical conclusion from this page alone was:

- a thin earlier chunked shim is possible,
- but for large-scale hot-path performance in single-worker mode it is materially worse than the contiguous arena path.

That conclusion was then refined by the multi-worker follow-up below.

---

# 16. Polars bank-chunked workers

## Prototype

- `polars-bank-chunked-workers.html`
- `polars-bank-chunked-shim.html`
- `polars-partition-arena-workers.html`
- `polars-spike/src/lib.rs`
- `polars-web/polars_spike.js`
- `polars-web/polars_spike_bg.wasm`

## Goal

Answer the missing question after the single-worker chunked-shim test:

- if the segmented chunked layout is distributed across multiple workers,
- does the chunked shim remain much worse than the partition-arena path,
- or was the earlier slowdown mostly a single-worker artifact?

This page compares three distributed paths on equal worker count:

1. generated baseline,
2. worker-local contiguous ownership-transfer arena,
3. worker-local earlier chunked shim.

## Manual run A — 100,000,000 rows, 4 workers

Configuration:

- total rows: `100,000,000`
- workers: `4`
- rows per worker: `25,000,000`
- segment rows per worker: `5,000,000`
- chunked segments per worker: `5`

Observed result:

- generated wall: `5283.6 ms`
- arena wall: `3473.1 ms`
- chunked wall: `5936.0 ms`
- arena max Polars: `3300.9 ms`
- chunked max Polars: `5749.1 ms`
- correctness: pass

## Manual run B — 100,000,000 rows, 8 workers

Configuration:

- total rows: `100,000,000`
- workers: `8`
- rows per worker: `12,500,000`
- segment rows per worker: `5,000,000`
- chunked segments per worker: `3`

Observed result:

- generated wall: `2395.3 ms`
- arena wall: `1932.4 ms`
- chunked wall: `3307.0 ms`
- arena max Polars: `1805.9 ms`
- chunked max Polars: `3121.9 ms`
- correctness: pass

## Interpretation

This experiment materially improves the interpretation of the earlier single-worker chunked-shim result.

The single-worker page was correct, but it overstated how bad the chunked path would look in a distributed design.

Under equal worker parallelism:

- the chunked path remains slower than the arena path,
- but it improves substantially,
- and it remains clearly viable.

At `100M` rows with `8` workers:

- arena wall: `1932.4 ms`
- chunked wall: `3307.0 ms`

So the chunked path is still about `1.7x` slower in wall time, but no longer looks like an outright rejection of the earlier-shim idea.

## Conclusion

The refined architectural conclusion is:

- **fastest hot path:**
  - `shard bank -> partition arena -> Polars`
- **thinner but slower path:**
  - `shard bank -> chunked shim -> Polars`

So the chunked shim is now best understood as:

- a credible thinner integration path,
- a correctness-proven earlier adapter into Polars,
- but not the preferred layout for peak throughput.

This means the architecture can legitimately support both:

1. **arena path** for the hottest workloads,
2. **chunked shim** for simpler integration or convenience-oriented workloads where some performance loss is acceptable.

---

# 17. Polars Arrow-view live workers

## Prototype

- `polars-arrow-view-live-workers.html`
- `polars-arrow-view-shardbank-workers.html`
- `polars-spike/src/lib.rs`
- `polars-web/polars_spike.js`
- `polars-web/polars_spike_bg.wasm`

## Goal

Take the next step after the live shared-bank manifest proof:

- keep the shared shard bank as the source of truth,
- keep the Arrow-style partition manifest,
- run one fresh-worker live-view aggregate directly from the bank,
- run one fresh-worker Polars-from-bank aggregate that builds worker-local arenas from the real manifest,
- and verify that both paths observe an in-bank mutation exactly.

This is still not direct external-memory Polars aliasing. It is the strongest currently implementable proof that Polars workers can consume the real bank-backed data path rather than a regenerated synthetic equivalent.

## Manual run used

Configuration:

- total rows: `38,500,000`
- partition rows: `2,048,000`
- workers: `4`
- shard size: `256 MiB`
- shards: `3`
- bank size: `768 MiB`
- used bytes: `599 MiB` displayed in UI / `628,582,912` bytes in result payload
- partitions: `19`
- mutation delta: `1000`

## Observed result

Initial runs:

- live-view wall: `95.7 ms`
- Polars-from-bank wall: `1175.5 ms`
- Polars init max: `102.3 ms`
- Polars fill max: `96.5 ms`
- Polars compute max: `1013.0 ms`
- live sum: `2,685,375,134`
- Polars sum: `2,685,375,134`
- JS reference sum: `2,685,375,134`

Mutation proof:

- mutated bank value at global row `0`
- `feature_a`: `-100 -> 900`
- expected delta: `1000`
- observed live delta: `1000`
- observed Polars delta: `1000`

Correctness:

- initial live path: pass
- initial Polars path: pass
- mutation tracking for live path: pass
- mutation tracking for Polars path: pass

## Interpretation

This is one of the strongest architectural integration proofs in the report.

It shows that:

1. **The shared shard bank is the real source of truth.**
   - A fresh-worker live-view path and a fresh-worker Polars-from-bank path both observed the exact in-bank mutation.

2. **Polars can consume the real bank-backed manifest path correctly.**
   - The Polars workers did not regenerate the synthetic pattern independently.
   - They rebuilt their local arenas from the actual bank partitions assigned through the manifest.

3. **The manifest/view layer is very cheap.**
   - The live-view wall time (`95.7 ms`) is tiny relative to the Polars path and strongly suggests the bank -> manifest -> fresh worker reconstruction layer is mostly metadata work.

4. **Direct external-memory Polars aliasing is no longer required to justify the backend.**
   - Even without proving true direct Polars aliasing of external shared-bank memory, the practical path is already strong:
   - `bank -> manifest -> worker-local arena -> Polars`

## Conclusion

This experiment substantially de-risks the recommended backend design.

The architecture can now be described more concretely as:

- **shared sharded Wasm-memory bank** as source of truth,
- **Arrow-style partition manifest** as the thin adapter layer,
- **fresh worker reconstruction** from that manifest,
- **direct live-view scans** for the lowest-level path,
- **Polars-from-bank worker arenas** for the higher-level dataframe path.

That is not yet the same as proving direct external-memory Polars aliasing, but it is strong enough to support the practical architecture choice.

---

# 18. Parquet -> bank -> Polars live workers

## Prototype

- `parquet-polars-bank-live-workers.html`
- `parquet-arrow-analytics.html`
- `polars-arrow-view-live-workers.html`
- `polars-spike/src/lib.rs`
- `polars-web/polars_spike.js`
- `polars-web/polars_spike_bg.wasm`

## Goal

Run the strongest practical end-to-end browser proof in the repo:

1. scan a real Parquet file in DuckDB-Wasm,
2. route selected typed columns into the shared Wasm-memory bank,
3. run fresh-worker live aggregates directly from the routed bank,
4. run fresh-worker Polars-from-bank aggregates over the same routed bank,
5. mutate one value in the routed bank,
6. verify that both paths observe the exact delta.

This is not just a synthetic bank demo. It is the real file-ingest -> bank -> worker-consumption path.

## Manual run used

Configuration:

- file: `synthetic-6gib.parquet`
- query: `SELECT partition, feature_a, label FROM parquet_scan('selected.parquet');`
- shard size: `256 MiB`
- target bank: `12 GiB`
- actual bank: `48` shards / `12.0 GiB`
- workers: `4`
- routed batches: `18,799`
- routed rows: `38,500,000`
- routed typed bytes: `587 MiB` displayed in UI / `616,000,000` bytes in result payload
- mutation delta: `1000`

## Observed result

Routing:

- route time: `6939.1 ms`

Initial worker runs:

- live wall: `140.8 ms`
- Polars wall: `825.2 ms`
- Polars init max: `98.6 ms`
- Polars fill max: `105.6 ms`
- Polars compute max: `621.1 ms`
- live sum: `2335.3217`
- Polars sum: `2335.3217`
- DuckDB sum: `2335.3217`

Mutation proof:

- mutated routed bank value at global row `3`
- `feature_a`: `0.4940390702158111 -> 1000.4940390702158`
- expected delta: `1000`
- observed live delta: `1000.0000000002588`
- observed Polars delta: `1000.0000000002588`

Correctness:

- live path: pass
- Polars path: pass
- mutation tracking through routed bank: pass

## Interpretation

This is the strongest practical passing experiment in the report.

It proves, on the real file path, that:

1. **DuckDB-Wasm -> typed routing -> shared bank is a valid ingest path.**
2. **Fresh workers can consume the routed bank directly.**
3. **Fresh Polars workers can consume the routed bank via the manifest -> arena path.**
4. **The routed bank remains the true source of truth after ingestion.**
   - Both the live worker path and the Polars path observed the exact in-bank mutation.

This result is especially important because it removes the last serious doubt that the recommended backend might only be working on synthetic standalone proofs.

## Conclusion

The practical backend architecture is now strongly supported by end-to-end evidence:

- **DuckDB-Wasm** for scan / file ingest / SQL entry,
- **shared sharded Wasm-memory bank** as the source-of-truth substrate,
- **thin manifest/view layer** for worker reconstruction,
- **live worker scans / Rust kernels** for the lowest-level hot path,
- **Polars-from-bank worker arenas** for the higher-level dataframe path.

This is strong enough that a much more invasive direct external-memory Polars aliasing proof is no longer required to justify the architecture.

---

# Final technical conclusions

## Proven strongly

The following claims are supported by successful runs in this repo:

1. **A Chromium tab can retain a multi-gigabyte sharded shared-memory bank.**
   - Demonstrated at `6 GiB` with `memory-bank.html`.

2. **Multiple workers can access a single SAB without memory scaling by worker count.**
   - Demonstrated with `index.html`.

3. **A long-running browser compute job can remain effectively unthrottled for 10 minutes when anchored appropriately.**
   - Demonstrated with `mvt2-persistence.html`.

4. **A local multi-gigabyte Parquet file can be scanned in DuckDB-Wasm, routed as Arrow/typed data into the shard bank, and processed in distributed workers.**
   - Demonstrated with `parquet-arrow-bank.html`, `parquet-arrow-analytics.html`.

5. **Rust/Wasm can read data zero-copy from a shared `WebAssembly.Memory` shard bank.**
   - Demonstrated with `rust-zero-copy-shardbank.html`.

6. **Rust/Wasm can compute correct typed aggregates directly from the shard bank.**
   - Demonstrated with `rust-typed-analytics.html`.

7. **Polars is feasible in browser Wasm and remains useful at large row counts.**
   - Demonstrated with `polars-spike.html`, `polars-typed-bridge.html`, `polars-shardbank-bridge.html`.

8. **Polars performs materially better when it can adopt already-populated, Polars-friendly buffers by ownership transfer.**
   - Demonstrated with `polars-zero-copy-microproof.html` at `10M` and `100M` rows.

9. **A monolithic shared Wasm linear memory is not a viable 10 GiB+ compute substrate in the tested browser environment.**
   - Demonstrated with `monolithic-memory-proof.html`.
   - Verified at `4 GiB`, then failed above that with the observed `65536`-page upper bound.
   - Therefore the compute substrate must be sharded for the target size class.

10. **Polars can consume segmented bank-like chunked columns through an earlier shim.**
   - Demonstrated with `polars-bank-chunked-shim.html` and `polars-bank-chunked-workers.html`.
   - Correctness passed in both single-worker and multi-worker forms.

11. **The chunked-shim path is slower than the contiguous arena path, but remains viable under equal worker parallelism.**
   - Demonstrated with `polars-bank-chunked-workers.html`.
   - The arena path remains the preferred hot path, but the chunked shim is now a credible thinner-integration alternative.

12. **Fresh Polars workers can consume the real shared-bank manifest path and track in-bank mutations exactly.**
   - Demonstrated with `polars-arrow-view-live-workers.html`.
   - This substantially de-risks `bank -> manifest -> worker-local arena -> Polars` as a practical architecture.

13. **The full practical path from real Parquet scan to routed bank to fresh live workers and fresh Polars workers works end to end.**
   - Demonstrated with `parquet-polars-bank-live-workers.html`.
   - Both the live path and the Polars path matched DuckDB and tracked an exact mutation through the routed real bank.

## Not yet proven / currently failing

1. **Rust Arrow IPC parsing directly from the shard bank is not validated.**
   - `rust-arrow-ipc-analytics.html` was too slow to be a clean pass.
   - `rust-arrow-ipc-diagnostic.html` failed with `unreachable` on a tiny subset.

2. **A single giant browser/Wasm arena for billion-row Polars ownership transfer is not yet practical in this prototype.**
   - `polars-zero-copy-microproof.html` failed with `unreachable` at `1,000,000,000` rows.
   - This looks like a practical memory-limit result, not a contradiction of the smaller successful zero-copy runs.

## Best current architectural interpretation

The strongest current architecture is:

- **DuckDB-Wasm** for scan / parquet decode / SQL
- **shared Wasm-memory shard bank** as the large browser-native memory substrate
- **typed manifests / typed segment routing** as the working structured bridge
- **custom Rust kernels** for hot-path zero-copy compute
- **Polars-friendly partition arenas** as the staging layer when a DataFrame engine is needed
- **Polars** as a higher-level dataframe layer that should preferably consume ownership-friendly contiguous buffers rather than arbitrary fragmented bank storage

This now rests on four stronger constraints than earlier in the report:

- a single monolithic shared Wasm memory is not enough for the target `10 GiB+` size class,
- so the architecture must assume sharding at the substrate level,
- while an earlier chunked shim into Polars is correct and viable, contiguous arenas are still faster,
- so the architecture should prefer **bank -> arena -> Polars** for hot paths while still allowing **bank -> chunked shim -> Polars** as a thinner alternative,
- the bank -> manifest -> fresh-worker -> arena -> Polars path is directly validated by mutation tracking over the shared bank,
- and the real file path `Parquet -> DuckDB -> bank -> live/Polars workers` is now validated end to end.

## Practical recommendation

If implementing Phase 2 now, the safest path is:

1. use the shard bank as the primary substrate,
2. use DuckDB-Wasm for file ingest / scan / SQL entry and route typed columns into the bank,
3. use custom Rust kernels or direct live worker scans for the core hot-path compute engine,
4. build contiguous per-partition column arenas when handing data to Polars for the hottest dataframe workloads,
5. place Polars above that arena layer for dataframe-style transforms and convenience workloads,
6. optionally support earlier chunked-shim ingestion into Polars as a thinner alternative when some performance loss is acceptable,
7. standardize on a thin manifest/view layer so fresh workers can rebuild their local execution state directly from the shared bank,
8. do **not** make Rust Arrow IPC parsing a required dependency path yet.

---

# File index referenced by this report

## Core prototypes

- `index.html`
- `wasm-workers.html`
- `memory-bank.html`
- `mvt2-persistence.html`
- `parquet-arrow-bank.html`
- `parquet-arrow-analytics.html`
- `rust-zero-copy-shardbank.html`
- `rust-typed-analytics.html`
- `rust-arrow-ipc-analytics.html`
- `rust-arrow-ipc-diagnostic.html`
- `polars-spike.html`
- `polars-typed-bridge.html`
- `polars-shardbank-bridge.html`
- `polars-zero-copy-microproof.html`
- `monolithic-memory-proof.html`
- `polars-bank-chunked-shim.html`
- `polars-bank-chunked-workers.html`
- `polars-arrow-view-live-workers.html`
- `parquet-polars-bank-live-workers.html`

## Rust / Wasm sources

- `wasm/sum_range.rs`
- `wasm/checksum_range.rs`
- `wasm/typed_aggregates.rs`
- `rust-arrow-ipc/src/lib.rs`
- `polars-spike/src/lib.rs`

## Built wasm assets

- `wasm/sum_range.wasm`
- `wasm/checksum_range.wasm`
- `wasm/typed_aggregates.wasm`
- `wasm/rust_arrow_ipc.wasm`
- `polars-web/polars_spike_bg.wasm`

## Data / support files

- `data/synthetic-6gib.parquet`
- `scripts/generate_synthetic_parquet.py`
- `serve.mjs`

---

# Bottom line

This repository now contains working browser evidence for:

- multi-gigabyte shared memory,
- multi-worker sustained compute,
- anti-throttled long-running execution,
- local Parquet scanning in DuckDB-Wasm,
- distributed analytics over a sharded bank,
- zero-copy Rust reads and typed Rust aggregates,
- practical Polars execution,
- shard-bank-to-Polars copy bridging,
- a Polars ownership-transfer micro-proof showing that Polars benefits from a Polars-friendly arena layout,
- a monolithic-memory proof showing that a single shared Wasm linear memory tops out at `4 GiB` in the tested environment, so the compute substrate must be sharded for `10 GiB+` targets,
- chunked-shim proofs showing that Polars can consume segmented bank-like columns correctly, and that while this earlier shim is slower than contiguous arena ingestion, it remains viable under equal worker parallelism,
- a live-manifest Polars proof showing that fresh Polars workers can consume the real shared-bank path and track in-bank mutations exactly,
- and a real-file end-to-end proof showing that Parquet -> DuckDB -> routed bank -> fresh live/Polars workers works and preserves exact mutation tracking through the routed bank.

The project is no longer at the “can this work at all?” stage. The remaining questions are primarily about **which layer should own which workload** and **how data should be laid out when crossing into higher-level engines like Polars**, not whether the browser can support the system in principle.
