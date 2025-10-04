// Hello World Capsule Template
// Target: 3-5KB WASM when compiled with -Oz

// Simple greeting function that demonstrates:
// - JSON input/output 
// - Host function usage (hash_commit)
// - Deterministic execution

export function run(input_ptr: i32, input_len: i32): i32 {
    // Read input from WASM memory
    const input_bytes = new Uint8Array(input_len);
    for (let i = 0; i < input_len; i++) {
        input_bytes[i] = load<u8>(input_ptr + i);
    }
    
    // Parse JSON input
    const input_str = String.fromCharCode.apply(null, Array.from(input_bytes));
    // Simple parsing - in real implementation would use JSON parser
    const name = extractName(input_str);
    
    // Create greeting
    const greeting = `Hello, ${name}! Greetings from Tenzik capsule.`;
    
    // Create deterministic output
    const output = `{"greeting": "${greeting}", "timestamp": "2024-12-01", "capsule": "hello-world"}`;
    
    // Convert to bytes
    const output_bytes = new Uint8Array(output.length);
    for (let i = 0; i < output.length; i++) {
        output_bytes[i] = output.charCodeAt(i);
    }
    
    // Store in WASM memory and return pointer
    const output_ptr = heap.alloc(output_bytes.length);
    for (let i = 0; i < output_bytes.length; i++) {
        store<u8>(output_ptr + i, output_bytes[i]);
    }
    
    // Return length in high bits, pointer in low bits
    return (output_bytes.length << 16) | output_ptr;
}

function extractName(input: string): string {
    // Simple name extraction from {"name": "value"}
    const start = input.indexOf('"name"');
    if (start === -1) return "World";
    
    const colonIdx = input.indexOf(':', start);
    const quote1 = input.indexOf('"', colonIdx);
    const quote2 = input.indexOf('"', quote1 + 1);
    
    if (quote1 === -1 || quote2 === -1) return "World";
    
    return input.substring(quote1 + 1, quote2);
}

// Export memory for host to access
export const memory = new WebAssembly.Memory({ initial: 1 });
