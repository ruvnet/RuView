# @ruvector/rvf-wasm

RuVector Format (RVF) WASM build for browsers and edge functions. Query vectors directly in the browser with zero backend.

## Install

```bash
npm install @ruvector/rvf-wasm
```

## Usage

```html
<script type="module">
  import init, { WasmRvfStore } from '@ruvector/rvf-wasm';
  await init();

  const store = WasmRvfStore.create(384);
  store.ingest(1, new Float32Array(384));
  const results = store.query(new Float32Array(384), 10);
  console.log(results); // [{ id, distance }]
</script>
```

## Features

- ~46 KB control plane (full store API)
- ~5.5 KB tile microkernel (query-only)
- In-memory store with HNSW indexing
- Segment inspection and status
- No backend required

## License

MIT
