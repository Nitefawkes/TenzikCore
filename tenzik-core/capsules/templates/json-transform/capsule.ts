// JSON Transform Capsule Template
// Target: 3-5KB WASM when compiled with -Oz

// Demonstrates JSON transformation and path extraction
// Use case: Webhook payload transformation, data mapping, field extraction

export function run(input_ptr: i32, input_len: i32): i32 {
    // Read input from WASM memory
    const input_bytes = new Uint8Array(input_len);
    for (let i = 0; i < input_len; i++) {
        input_bytes[i] = load<u8>(input_ptr + i);
    }

    // Convert to string
    const input_str = String.fromCharCode.apply(null, Array.from(input_bytes));

    // Extract fields from input JSON
    // Example input: {"user": {"name": "Alice", "age": 30}, "action": "login", "timestamp": 1234567890}
    const user_name = extractJsonValue(input_str, "user.name");
    const action = extractJsonValue(input_str, "action");
    const timestamp = extractJsonValue(input_str, "timestamp");

    // Transform data - create a new structure
    // Example: Convert user login event to notification format
    const output = buildJsonOutput(user_name, action, timestamp);

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

// Extract JSON value using simple path notation (e.g., "user.name")
function extractJsonValue(json: string, path: string): string {
    // Split path into parts
    const parts = path.split('.');

    if (parts.length === 1) {
        // Simple field extraction
        return extractSimpleField(json, parts[0]);
    } else if (parts.length === 2) {
        // Nested object.field extraction
        const objValue = extractSimpleField(json, parts[0]);
        if (objValue === "null") return "null";
        return extractSimpleField(objValue, parts[1]);
    }

    return "null";
}

// Extract a simple field from JSON string
// Handles both string values ("field": "value") and numeric values ("field": 123)
function extractSimpleField(json: string, field: string): string {
    const pattern = `"${field}"`;
    const start = json.indexOf(pattern);
    if (start === -1) return "null";

    const colonIdx = json.indexOf(':', start);
    if (colonIdx === -1) return "null";

    // Skip whitespace after colon
    let valueStart = colonIdx + 1;
    while (valueStart < json.length && (json.charAt(valueStart) === ' ' || json.charAt(valueStart) === '\t')) {
        valueStart++;
    }

    const firstChar = json.charAt(valueStart);

    if (firstChar === '"') {
        // String value
        const quote2 = json.indexOf('"', valueStart + 1);
        if (quote2 === -1) return "null";
        return json.substring(valueStart + 1, quote2);
    } else if (firstChar === '{') {
        // Object value - find matching closing brace
        let braceCount = 0;
        let i = valueStart;
        while (i < json.length) {
            if (json.charAt(i) === '{') braceCount++;
            if (json.charAt(i) === '}') {
                braceCount--;
                if (braceCount === 0) {
                    return json.substring(valueStart, i + 1);
                }
            }
            i++;
        }
        return "null";
    } else {
        // Numeric or boolean value - read until comma, brace, or bracket
        let i = valueStart;
        while (i < json.length) {
            const ch = json.charAt(i);
            if (ch === ',' || ch === '}' || ch === ']' || ch === ' ' || ch === '\t' || ch === '\n') {
                return json.substring(valueStart, i).trim();
            }
            i++;
        }
        return json.substring(valueStart).trim();
    }
}

// Build output JSON
function buildJsonOutput(userName: string, action: string, timestamp: string): string {
    // Create transformed output structure
    // Original: login event
    // Transformed: notification event with enriched data

    const message = `User ${userName} performed ${action}`;
    const severity = action === "login" ? "info" : "warning";

    return `{"event":"notification","message":"${message}","user":"${userName}","action":"${action}","severity":"${severity}","timestamp":${timestamp},"processed_by":"json-transform-capsule"}`;
}

// Export memory for host to access
export const memory = new WebAssembly.Memory({ initial: 1 });
