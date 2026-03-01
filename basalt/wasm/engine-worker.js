// =============================================================================
// Basalt Engine Worker — v0.4.0 Clean WASM Bridge
// =============================================================================
// JS owns: BPE tokenization, string decoding, Memory allocation
// WASM owns: math (forward passes, sampling, RoPE).

// ── Float bit-casting helper ────────────────────────────────────────────────
const _f32buf = new ArrayBuffer(4);
const _f32view = new Float32Array(_f32buf);
const _i32view = new Int32Array(_f32buf);
function floatBitsToInt(f) { _f32view[0] = f; return _i32view[0]; }

let wasm = null;
let vocab = null;
let vocabDecode = null;
let running = false;
let memory = null;
let promptBufferPtr = 0; // Reusable buffer for prompt tokens

// ── BPE Tokenizer ───────────────────────────────────────────────────────────

async function loadTokenizer(url) {
    const response = await fetch(url);
    const buffer = await response.arrayBuffer();
    const view = new DataView(buffer);
    let offset = 0;

    const maxTokenLen = view.getInt32(offset, true); offset += 4;
    const vocabSize = view.getInt32(offset, true); offset += 4;

    vocab = new Map();
    vocabDecode = new Map();

    for (let i = 0; i < vocabSize; i++) {
        const score = view.getFloat32(offset, true); offset += 4;
        const len = view.getInt32(offset, true); offset += 4;
        const bytes = new Uint8Array(buffer, offset, len); offset += len;
        const text = new TextDecoder().decode(bytes);
        vocab.set(text, i);
        vocabDecode.set(i, text);
    }
}

function encodePrompt(text) {
    const tokens = [];
    tokens.push(1); // BOS

    // Build special-token regex from vocab entries matching <|...|> pattern
    const specialTokens = [];
    for (const [key, id] of vocab.entries()) {
        if (/^<\|.*\|>$/.test(key)) {
            specialTokens.push({ text: key, id });
        }
    }

    // Split input into [non-special, special, non-special, ...] segments
    // Sort by length descending to match longest special tokens first
    specialTokens.sort((a, b) => b.text.length - a.text.length);

    const escapeRegex = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const pattern = specialTokens.length > 0
        ? new RegExp(`(${specialTokens.map(s => escapeRegex(s.text)).join('|')})`)
        : null;

    const segments = pattern ? text.split(pattern) : [text];

    for (const segment of segments) {
        if (!segment) continue;

        // Check if this segment is a special token
        const specialMatch = specialTokens.find(s => s.text === segment);
        if (specialMatch) {
            tokens.push(specialMatch.id);
            continue;
        }

        // Normal segment: greedy substring matching
        const chars = [...segment];
        const pieces = chars.map(c => vocab.get(c) ?? 0);

        let i = 0;
        while (i < chars.length) {
            let bestLen = 1;
            let bestId = pieces[i];

            for (let len = 2; len <= Math.min(20, chars.length - i); len++) {
                const substr = chars.slice(i, i + len).join('');
                const id = vocab.get(substr);
                if (id !== undefined) {
                    bestLen = len;
                    bestId = id;
                }
            }
            tokens.push(bestId);
            i += bestLen;
        }
    }
    return tokens;
}

// ── WASM Lifecycle ──────────────────────────────────────────────────────────

async function loadModel(modelUrl, tokenizerUrl) {
    postMessage({ type: 'STATUS', message: 'Loading model & tokenizer...' });

    const [modelResp, _] = await Promise.all([
        fetch(modelUrl),
        tokenizerUrl ? loadTokenizer(tokenizerUrl) : Promise.resolve()
    ]);

    const modelBytes = new Uint8Array(await modelResp.arrayBuffer());

    // 1. Create a large enough Memory object (163MB model needs ~3000 pages)
    // 256 pages = 16MB. We allocate 10000 pages (~640MB) to be safe for 15M param models.
    memory = new WebAssembly.Memory({ initial: 10000, maximum: 10000 });

    const importObject = {
        env: {
            memory: memory,
            log_status: (ptr, len) => {
                const bytes = new Uint8Array(memory.buffer, ptr, len);
                const text = new TextDecoder().decode(bytes);
                console.log('[basalt]', text);
            }
        }
    };

    postMessage({ type: 'STATUS', message: 'Initializing WASM...' });
    const wasmResponse = await fetch('/basalt.wasm');
    const wasmModule = await WebAssembly.instantiateStreaming(wasmResponse, importObject);
    wasm = wasmModule.instance;

    // 2. Copy model into WASM memory
    const modelPtr = Number(wasm.exports.basalt_alloc(BigInt(modelBytes.byteLength)));
    new Uint8Array(memory.buffer, modelPtr, modelBytes.byteLength).set(modelBytes);

    // 3. Initialize engine
    const status = wasm.exports.basalt_init(modelPtr, BigInt(modelBytes.byteLength));
    if (status !== 0) throw new Error('basalt_init failed');

    // 4. Pre-allocate a 4KB reusable prompt buffer (8 bytes per token * 512 tokens max)
    // Avoids calling basalt_alloc on every prompt.
    promptBufferPtr = Number(wasm.exports.basalt_alloc(4096n));

    postMessage({ type: 'READY' });
}

async function runPrompt(prompt, maxNewTokens = 128, temperature = 0.0, topP = 0.9, seed = 0) {
    if (!wasm) throw new Error('Model not loaded');
    running = true;

    // Set sampling parameters before generation
    const actualSeed = seed || Date.now();
    wasm.exports.basalt_set_sampling(
        floatBitsToInt(temperature),
        floatBitsToInt(topP),
        BigInt(actualSeed)
    );

    postMessage({ type: 'STATUS', message: 'Tokenizing prompt...' });
    const tokens = encodePrompt(prompt);

    // Write tokens into the pre-allocated reusable buffer
    const maxTokensBuffer = 4096 / 8; // 512
    const tokenCount = Math.min(tokens.length, maxTokensBuffer);

    const tokensView = new BigInt64Array(memory.buffer, promptBufferPtr, tokenCount);
    for (let i = 0; i < tokenCount; i++) {
        tokensView[i] = BigInt(tokens[i]);
    }

    postMessage({ type: 'STATUS', message: `Prefilling ${tokenCount} tokens...` });
    wasm.exports.basalt_ingest_prompt(promptBufferPtr, BigInt(tokenCount));

    postMessage({ type: 'STATUS', message: 'Generating...' });
    const startMs = performance.now();
    let totalTokens = 0;

    for (let step = 0; step < maxNewTokens; step++) {
        if (!running) break;

        const tokenId = Number(wasm.exports.basalt_generate_next());
        if (tokenId < 0) break; // EOS

        totalTokens++;
        const text = vocabDecode?.get(tokenId) ?? `<${tokenId}>`;
        postMessage({ type: 'TOKEN', tokenId, text });

        // Yield to event loop every N tokens for responsiveness
        if (step % 4 === 0) await new Promise(r => setTimeout(r, 0));
    }

    const elapsedMs = performance.now() - startMs;
    postMessage({ type: 'DONE', totalTokens, elapsedMs });
    running = false;
}

self.onmessage = async (e) => {
    try {
        if (e.data.type === 'LOAD_MODEL') await loadModel(e.data.modelUrl, e.data.tokenizerUrl);
        else if (e.data.type === 'RUN_PROMPT') await runPrompt(
            e.data.prompt,
            e.data.maxNewTokens,
            e.data.temperature ?? 0.0,
            e.data.topP ?? 0.9,
            e.data.seed ?? 0
        );
        else if (e.data.type === 'STOP') running = false;
        else if (e.data.type === 'RESET') {
            if (wasm) wasm.exports.basalt_reset();
            postMessage({ type: 'STATUS', message: 'Context reset (KV cache cleared)' });
        }
    } catch (err) {
        console.error(err);
        postMessage({ type: 'ERROR', message: err.message });
    }
};
