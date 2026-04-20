import { readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { resolve } from "node:path";

const [candidateArg, baselineArg] = process.argv.slice(2);

if (!candidateArg || !baselineArg) {
  console.error(
    "Usage: node scripts/benchmark-wasm.mjs <candidate.wasm> <baseline.wasm>",
  );
  process.exit(1);
}

const PAYLOADS = {
  ascii4k: Buffer.from("A".repeat(4096)),
  ansi4k: Buffer.from(
    (
      Array.from(
        { length: 64 },
        (_, i) =>
          `\x1b[${31 + (i % 7)}mline-${String(i).padStart(2, "0")} lorem ipsum dolor sit amet\r\n`,
      ).join("") + "\x1b[0m"
    ).repeat(2),
  ),
  mixed16k: Buffer.from(
    (
      Array.from(
        { length: 200 },
        (_, i) =>
          `\x1b[${
            i % 2 ? "1;34" : "38;5;208"
          }mitem-${i}\x1b[0m `,
      ).join("") + "\r\n"
    ).repeat(4),
  ),
};

const CASES = [
  ["ascii4k", 5000, (mod) => writePayload(mod, PAYLOADS.ascii4k, 5000)],
  ["ansi4k", 2500, (mod) => writePayload(mod, PAYLOADS.ansi4k, 2500)],
  ["mixed16k", 800, (mod) => writePayload(mod, PAYLOADS.mixed16k, 800)],
  ["resize", 50000, (mod) => resizeChurn(mod, 50000)],
];

async function loadModule(label, file) {
  const path = resolve(file);
  const bytes = readFileSync(path);
  const { instance } = await WebAssembly.instantiate(bytes);
  return { label, path, size: bytes.length, exports: instance.exports };
}

function writePayload(mod, data, iterations) {
  const { exports } = mod;
  const memory = new Uint8Array(exports.memory.buffer);
  const ptr = exports.getWriteBuffer();
  let checksum = 0;
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    exports.init(80, 24);
    memory.set(data, ptr);
    exports.writeBytes(data.length);
    checksum ^=
      exports.getCursorCol() +
      (exports.getCursorRow() << 8) +
      exports.getScrollbackCount();
  }
  return { ms: performance.now() - start, checksum };
}

function resizeChurn(mod, iterations) {
  const { exports } = mod;
  exports.init(80, 24);
  let checksum = 0;
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    exports.resizeTerminal(80 + (i % 17), 24 + (i % 9));
    checksum ^= exports.getCols() + (exports.getRows() << 8);
  }
  return { ms: performance.now() - start, checksum };
}

function pctDelta(candidateMs, baselineMs) {
  return ((candidateMs / baselineMs) - 1) * 100;
}

const [candidate, baseline] = await Promise.all([
  loadModule("candidate", candidateArg),
  loadModule("baseline", baselineArg),
]);

console.log(
  JSON.stringify(
    {
      candidate: { path: candidate.path, size: candidate.size },
      baseline: { path: baseline.path, size: baseline.size },
    },
    null,
    2,
  ),
);

for (const [name, iterations, runCase] of CASES) {
  const baselineResult = runCase(baseline);
  const candidateResult = runCase(candidate);
  console.log(
    JSON.stringify(
      {
        name,
        iterations,
        baseline_ms: baselineResult.ms,
        candidate_ms: candidateResult.ms,
        candidate_vs_baseline_pct: pctDelta(
          candidateResult.ms,
          baselineResult.ms,
        ),
        checksums: {
          baseline: baselineResult.checksum,
          candidate: candidateResult.checksum,
        },
      },
      null,
      2,
    ),
  );
}
