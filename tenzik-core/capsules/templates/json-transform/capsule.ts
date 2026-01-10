// JSON Transform Capsule Template
// Target: <4KB WASM when compiled with -Oz
// Demonstrates: Field selection, renaming, transformations

export function run(input_ptr: i32, input_len: i32): i32 {
    // Read input from WASM memory
    const input_bytes = new Uint8Array(input_len);
    for (let i = 0; i < input_len; i++) {
        input_bytes[i] = load<u8>(input_ptr + i);
    }

    // Parse JSON input (simple parser for demo)
    const input_str = String.fromCharCode.apply(null, Array.from(input_bytes));

    // Extract data and transform sections
    const data = extractSection(input_str, "data");
    const transform = extractSection(input_str, "transform");

    // Apply transformations
    const result = applyTransform(data, transform);

    // Create output with metadata
    const output = createOutput(result);

    // Convert to bytes
    const output_bytes = new Uint8Array(output.length);
    for (let i = 0; i < output.length; i++) {
        output_bytes[i] = output.charCodeAt(i);
    }

    // Allocate output in memory
    const output_ptr = heap.alloc(output_bytes.length);
    for (let i = 0; i < output_bytes.length; i++) {
        store<u8>(output_ptr + i, output_bytes[i]);
    }

    // Return length in high bits, pointer in low bits
    return (output_bytes.length << 16) | output_ptr;
}

function extractSection(input: string, section: string): string {
    const pattern = `"${section}"`;
    const start = input.indexOf(pattern);
    if (start === -1) return "{}";

    const colonIdx = input.indexOf(':', start);
    const braceIdx = input.indexOf('{', colonIdx);

    if (braceIdx === -1) return "{}";

    // Find matching closing brace
    let depth = 1;
    let i = braceIdx + 1;
    while (i < input.length && depth > 0) {
        if (input.charAt(i) === '{') depth++;
        if (input.charAt(i) === '}') depth--;
        i++;
    }

    return input.substring(braceIdx, i);
}

function applyTransform(data: string, transform: string): string {
    // Extract select fields
    const selectFields = extractArray(transform, "select");

    // Extract rename mappings
    const renameMap = extractObject(transform, "rename");

    // Extract operations
    const operations = extractObject(transform, "operations");

    // Build result object
    let result = "{";
    let fieldCount = 0;

    for (let i = 0; i < selectFields.length; i++) {
        const field = selectFields[i];
        const value = extractFieldValue(data, field);

        if (value !== "") {
            if (fieldCount > 0) result += ",";

            // Apply rename if exists
            let outputField = field;
            if (renameMap.has(field)) {
                outputField = renameMap.get(field);
            }

            // Apply operation if exists
            let outputValue = value;
            if (operations.has(outputField)) {
                const op = operations.get(outputField);
                outputValue = applyOperation(value, op);
            }

            result += `"${outputField}":"${outputValue}"`;
            fieldCount++;
        }
    }

    result += "}";
    return result;
}

function extractArray(json: string, key: string): string[] {
    const pattern = `"${key}"`;
    const start = json.indexOf(pattern);
    if (start === -1) return [];

    const colonIdx = json.indexOf(':', start);
    const bracketIdx = json.indexOf('[', colonIdx);
    if (bracketIdx === -1) return [];

    const closeBracket = json.indexOf(']', bracketIdx);
    const arrayStr = json.substring(bracketIdx + 1, closeBracket);

    const result: string[] = [];
    let current = "";
    let inQuote = false;

    for (let i = 0; i < arrayStr.length; i++) {
        const char = arrayStr.charAt(i);
        if (char === '"') {
            inQuote = !inQuote;
        } else if (char === ',' && !inQuote) {
            if (current.trim() !== "") {
                result.push(current.trim());
            }
            current = "";
        } else if (inQuote) {
            current += char;
        }
    }

    if (current.trim() !== "") {
        result.push(current.trim());
    }

    return result;
}

function extractObject(json: string, key: string): Map<string, string> {
    const result = new Map<string, string>();
    const pattern = `"${key}"`;
    const start = json.indexOf(pattern);
    if (start === -1) return result;

    const colonIdx = json.indexOf(':', start);
    const braceIdx = json.indexOf('{', colonIdx);
    if (braceIdx === -1) return result;

    // Find matching closing brace
    let depth = 1;
    let i = braceIdx + 1;
    while (i < json.length && depth > 0) {
        if (json.charAt(i) === '{') depth++;
        if (json.charAt(i) === '}') depth--;
        i++;
    }

    const objStr = json.substring(braceIdx + 1, i - 1);

    // Parse key-value pairs
    let currentKey = "";
    let currentValue = "";
    let inKey = false;
    let inValue = false;

    for (let j = 0; j < objStr.length; j++) {
        const char = objStr.charAt(j);

        if (char === '"' && !inValue) {
            inKey = !inKey;
        } else if (char === ':' && inKey) {
            inKey = false;
            // Skip to next quote
            j++;
            while (j < objStr.length && objStr.charAt(j) !== '"') j++;
            inValue = true;
        } else if (char === '"' && inValue) {
            result.set(currentKey.trim(), currentValue.trim());
            currentKey = "";
            currentValue = "";
            inValue = false;
        } else if (inKey) {
            currentKey += char;
        } else if (inValue) {
            currentValue += char;
        }
    }

    return result;
}

function extractFieldValue(data: string, field: string): string {
    // Handle nested fields (e.g., "user.profile.name")
    const parts = field.split('.');
    let current = data;

    for (let i = 0; i < parts.length; i++) {
        const part = parts[i];
        const pattern = `"${part}"`;
        const start = current.indexOf(pattern);

        if (start === -1) return "";

        const colonIdx = current.indexOf(':', start);
        const nextChar = current.charAt(colonIdx + 1);

        if (nextChar === '{') {
            // It's an object, continue nesting
            const braceIdx = colonIdx + 1;
            let depth = 1;
            let j = braceIdx + 1;
            while (j < current.length && depth > 0) {
                if (current.charAt(j) === '{') depth++;
                if (current.charAt(j) === '}') depth--;
                j++;
            }
            current = current.substring(braceIdx, j);
        } else {
            // It's a value
            const quote1 = current.indexOf('"', colonIdx);
            if (quote1 === -1) return "";
            const quote2 = current.indexOf('"', quote1 + 1);
            if (quote2 === -1) return "";
            return current.substring(quote1 + 1, quote2);
        }
    }

    return "";
}

function applyOperation(value: string, operation: string): string {
    if (operation === "uppercase") {
        return value.toUpperCase();
    } else if (operation === "lowercase") {
        return value.toLowerCase();
    } else if (operation === "truncate_10") {
        return value.length > 10 ? value.substring(0, 10) : value;
    }
    return value;
}

function createOutput(result: string): string {
    return `{"result":${result},"metadata":{"capsule":"json-transform","timestamp":"2024-12-01"}}`;
}

// Export memory for host to access
export const memory = new WebAssembly.Memory({ initial: 1 });
