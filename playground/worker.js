const ready = WebAssembly.instantiateStreaming(fetch("khipu.wasm"), {}).then(
  ({ instance }) => instance.exports
);

function call(khipu, name, source) {
  const bytes = new TextEncoder().encode(source);
  if (bytes.length > khipu.input_capacity()) {
    return "\u0000error: the program is too long for the playground";
  }
  const at = khipu.input_ptr();
  new Uint8Array(khipu.memory.buffer, at, bytes.length).set(bytes);
  const length = khipu[name](bytes.length);
  const result = khipu.result_ptr();
  return new TextDecoder().decode(new Uint8Array(khipu.memory.buffer, result, length));
}

self.onmessage = async ({ data }) => {
  const khipu = await ready;
  const started = performance.now();
  const result = call(khipu, data.op, data.source);
  self.postMessage({ op: data.op, id: data.id, result, ms: performance.now() - started });
};
