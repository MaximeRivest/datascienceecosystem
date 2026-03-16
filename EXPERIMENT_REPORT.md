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

## Not yet proven / currently failing

1. **Rust Arrow IPC parsing directly from the shard bank is not validated.**
   - `rust-arrow-ipc-analytics.html` was too slow to be a clean pass.
   - `rust-arrow-ipc-diagnostic.html` failed with `unreachable` on a tiny subset.

## Best current architectural interpretation

The strongest current architecture is:

- **DuckDB-Wasm** for scan / parquet decode / SQL
- **shared Wasm-memory shard bank** as the large browser-native memory substrate
- **typed manifests / typed segment routing** as the working structured bridge
- **custom Rust kernels** for hot-path zero-copy compute
- **Polars** as a promising higher-level, copy-bridge dataframe layer for transforms and convenience workloads

## Practical recommendation

If implementing Phase 2 now, the safest path is:

1. use the shard bank as the primary substrate,
2. route typed columns into it,
3. use custom Rust kernels for the core compute engine,
4. optionally place Polars above that as a convenience / dataframe layer,
5. do **not** make Rust Arrow IPC parsing a required dependency path yet.

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
- and practical Polars execution plus shard-bank bridge experiments.

The project is no longer at the “can this work at all?” stage. The remaining questions are primarily about **which layer should own which workload**, not whether the browser can support the system in principle.
