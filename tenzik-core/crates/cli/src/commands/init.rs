use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const HELLO_WORLD_CAPSULE: &str = r#"export function run(input_ptr: i32, input_len: i32): i32 {
    // Read input from WASM memory
    const input_bytes = new Uint8Array(input_len);
    for (let i = 0; i < input_len; i++) {
        input_bytes[i] = load<u8>(input_ptr + i);
    }

    const input_str = String.fromCharCode.apply(null, Array.from(input_bytes));
    const result = `Hello, ${input_str}!`;

    // Write output to WASM memory
    const output_bytes = new Uint8Array(result.length);
    for (let i = 0; i < result.length; i++) {
        output_bytes[i] = result.charCodeAt(i);
    }

    // Allocate and write output
    const output_ptr = heap.alloc(output_bytes.length) as i32;
    for (let i = 0; i < output_bytes.length; i++) {
        store<u8>(output_ptr + i, output_bytes[i]);
    }

    // Return pointer and length as packed i32
    return (output_bytes.length << 16) | output_ptr;
}
"#;

const HELLO_WORLD_README: &str = r#"# Hello World Capsule

A simple Tenzik capsule that demonstrates basic input/output handling.

## What it does

Takes a string input and returns "Hello, {input}!"

## Quick Start

### 1. Install AssemblyScript

```bash
npm install -g assemblyscript
```

### 2. Compile to WASM

```bash
npm run build
```

### 3. Test locally

```bash
tenzik test build/capsule.wasm "World"
```

Expected output:
```
Hello, World!
```

### 4. Validate the capsule

```bash
tenzik validate build/capsule.wasm
```

## File Structure

- `capsule.ts` - Main capsule code
- `package.json` - Node.js dependencies and build scripts
- `tsconfig.json` - TypeScript/AssemblyScript configuration

## Next Steps

- Modify `capsule.ts` to implement your logic
- Test with different inputs
- Deploy to a Tenzik node

## Resources

- [AssemblyScript Documentation](https://www.assemblyscript.org/)
- [Tenzik Documentation](../../docs/)
"#;

const HELLO_WORLD_PACKAGE_JSON: &str = r#"{
  "name": "tenzik-hello-world",
  "version": "1.0.0",
  "description": "Hello World Tenzik capsule",
  "scripts": {
    "build": "asc capsule.ts -o build/capsule.wasm --optimize --runtime stub",
    "test": "npm run build && tenzik test build/capsule.wasm \"World\""
  },
  "devDependencies": {
    "assemblyscript": "^0.27.0"
  }
}
"#;

const HELLO_WORLD_TSCONFIG: &str = r#"{
  "extends": "assemblyscript/std/assembly.json",
  "include": [
    "capsule.ts"
  ]
}
"#;

const JSON_TRANSFORM_CAPSULE: &str = r#"// JSON Transform Capsule
// Selects, renames, and transforms JSON fields

export function run(input_ptr: i32, input_len: i32): i32 {
    // Read input
    const input_bytes = new Uint8Array(input_len);
    for (let i = 0; i < input_len; i++) {
        input_bytes[i] = load<u8>(input_ptr + i);
    }

    const input_str = String.fromCharCode.apply(null, Array.from(input_bytes));

    // Extract data and transform sections
    const data = extractSection(input_str, "data");
    const transform = extractSection(input_str, "transform");

    // Apply transformations
    const result = applyTransform(data, transform);

    // Write output
    const output_bytes = new Uint8Array(result.length);
    for (let i = 0; i < result.length; i++) {
        output_bytes[i] = result.charCodeAt(i);
    }

    const output_ptr = heap.alloc(output_bytes.length) as i32;
    for (let i = 0; i < output_bytes.length; i++) {
        store<u8>(output_ptr + i, output_bytes[i]);
    }

    return (output_bytes.length << 16) | output_ptr;
}

function extractSection(input: string, section: string): string {
    const marker = `"${section}":`;
    const start = input.indexOf(marker);
    if (start === -1) return "{}";

    let depth = 0;
    let inString = false;
    let startBrace = -1;

    for (let i = start + marker.length; i < input.length; i++) {
        const char = input.charAt(i);

        if (char === '"') inString = !inString;
        if (inString) continue;

        if (char === '{') {
            if (depth === 0) startBrace = i;
            depth++;
        }
        if (char === '}') {
            depth--;
            if (depth === 0) {
                return input.substring(startBrace, i + 1);
            }
        }
    }

    return "{}";
}

function applyTransform(data: string, transform: string): string {
    const selectFields = extractArray(transform, "select");
    const renameMap = extractObject(transform, "rename");

    let result = "{";
    let first = true;

    // Process each selected field
    for (let i = 0; i < selectFields.length; i++) {
        const field = selectFields[i];
        const value = extractValue(data, field);

        if (value !== null) {
            if (!first) result += ",";

            // Check if field should be renamed
            const newName = getRenamed(renameMap, field) || field;
            result += `"${newName}":${value}`;
            first = false;
        }
    }

    result += "}";
    return result;
}

function extractArray(json: string, key: string): string[] {
    const result: string[] = [];
    const marker = `"${key}":`;
    const start = json.indexOf(marker);
    if (start === -1) return result;

    const bracketStart = json.indexOf('[', start);
    if (bracketStart === -1) return result;

    const bracketEnd = json.indexOf(']', bracketStart);
    if (bracketEnd === -1) return result;

    const arrayContent = json.substring(bracketStart + 1, bracketEnd);
    const items = arrayContent.split(',');

    for (let i = 0; i < items.length; i++) {
        let item = items[i].trim();
        if (item.startsWith('"') && item.endsWith('"')) {
            item = item.substring(1, item.length - 1);
        }
        if (item.length > 0) {
            result.push(item);
        }
    }

    return result;
}

function extractObject(json: string, key: string): string {
    return extractSection(json, key);
}

function extractValue(json: string, key: string): string | null {
    const marker = `"${key}":`;
    const start = json.indexOf(marker);
    if (start === -1) return null;

    let valueStart = start + marker.length;
    while (valueStart < json.length && json.charAt(valueStart) === ' ') {
        valueStart++;
    }

    const firstChar = json.charAt(valueStart);

    if (firstChar === '"') {
        const end = json.indexOf('"', valueStart + 1);
        if (end === -1) return null;
        return json.substring(valueStart, end + 1);
    }

    if (firstChar === '{' || firstChar === '[') {
        let depth = 1;
        let i = valueStart + 1;
        const endChar = firstChar === '{' ? '}' : ']';

        while (i < json.length && depth > 0) {
            if (json.charAt(i) === firstChar) depth++;
            if (json.charAt(i) === endChar) depth--;
            i++;
        }

        return json.substring(valueStart, i);
    }

    let i = valueStart;
    while (i < json.length) {
        const char = json.charAt(i);
        if (char === ',' || char === '}' || char === ']') break;
        i++;
    }

    return json.substring(valueStart, i).trim();
}

function getRenamed(renameMap: string, oldName: string): string | null {
    const marker = `"${oldName}":`;
    const start = renameMap.indexOf(marker);
    if (start === -1) return null;

    let valueStart = start + marker.length;
    while (valueStart < renameMap.length && renameMap.charAt(valueStart) === ' ') {
        valueStart++;
    }

    if (renameMap.charAt(valueStart) !== '"') return null;

    valueStart++;
    const end = renameMap.indexOf('"', valueStart);
    if (end === -1) return null;

    return renameMap.substring(valueStart, end);
}
"#;

const JSON_TRANSFORM_README: &str = r#"# JSON Transform Capsule

A Tenzik capsule for selecting, renaming, and transforming JSON fields.

## What it does

Transforms JSON data by:
- Selecting specific fields
- Renaming fields
- Extracting nested values

## Quick Start

### 1. Install dependencies

```bash
npm install
```

### 2. Build the capsule

```bash
npm run build
```

### 3. Test with example data

```bash
tenzik test build/capsule.wasm '{
  "data": {"name": "Alice", "age": 30, "email": "alice@example.com"},
  "transform": {
    "select": ["name", "email"],
    "rename": {"name": "fullName", "email": "contactEmail"}
  }
}'
```

Expected output:
```json
{"fullName":"Alice","contactEmail":"alice@example.com"}
```

## Input Format

```json
{
  "data": {
    // Your JSON data here
  },
  "transform": {
    "select": ["field1", "field2"],  // Fields to extract
    "rename": {                       // Optional field renaming
      "field1": "newName1"
    }
  }
}
```

## Use Cases

- API response transformation
- Data privacy (field filtering)
- Schema migration
- ETL pipelines
- Webhook payload transformation

## Advanced Usage

### Webhook Integration

Deploy this capsule with Tenzik's webhook router to create verifiable API transformations:

```rust
let config = WebhookConfig {
    port: 8080,
    capsule_path: PathBuf::from("build/capsule.wasm"),
    enable_zk_proofs: true,
    ..Default::default()
};
```

Every transformation produces a cryptographically signed receipt proving:
- Exact capsule used
- Input hash
- Output hash
- Execution metrics

## Resources

- [AssemblyScript Documentation](https://www.assemblyscript.org/)
- [Tenzik Webhook Guide](../../docs/guides/webhooks.md)
"#;

pub struct InitArgs {
    pub name: Option<String>,
    pub template: String,
}

pub fn execute_init_command(args: InitArgs) -> Result<()> {
    let project_name = args.name.unwrap_or_else(|| "my-capsule".to_string());
    let project_path = PathBuf::from(&project_name);

    // Check if directory already exists
    if project_path.exists() {
        anyhow::bail!("Directory '{}' already exists. Please choose a different name or remove the existing directory.", project_name);
    }

    println!("🚀 Creating new Tenzik capsule project: {}", project_name);
    println!("📦 Template: {}", args.template);

    // Create project directory
    fs::create_dir_all(&project_path)
        .context("Failed to create project directory")?;

    // Create build directory
    let build_dir = project_path.join("build");
    fs::create_dir_all(&build_dir)
        .context("Failed to create build directory")?;

    // Add .gitignore
    fs::write(project_path.join(".gitignore"), "build/\nnode_modules/\n")
        .context("Failed to create .gitignore")?;

    // Generate template files based on selected template
    match args.template.as_str() {
        "hello-world" => {
            create_hello_world_template(&project_path)?;
            println!("\n✅ Created hello-world capsule project!");
            print_next_steps(&project_name, "World");
        }
        "json-transform" => {
            create_json_transform_template(&project_path)?;
            println!("\n✅ Created json-transform capsule project!");
            print_next_steps_json(&project_name);
        }
        _ => {
            // Default to hello-world
            create_hello_world_template(&project_path)?;
            println!("\n⚠️  Unknown template '{}', using 'hello-world'", args.template);
            print_next_steps(&project_name, "World");
        }
    }

    Ok(())
}

fn create_hello_world_template(project_path: &Path) -> Result<()> {
    fs::write(project_path.join("capsule.ts"), HELLO_WORLD_CAPSULE)?;
    fs::write(project_path.join("README.md"), HELLO_WORLD_README)?;
    fs::write(project_path.join("package.json"), HELLO_WORLD_PACKAGE_JSON)?;
    fs::write(project_path.join("tsconfig.json"), HELLO_WORLD_TSCONFIG)?;
    Ok(())
}

fn create_json_transform_template(project_path: &Path) -> Result<()> {
    fs::write(project_path.join("capsule.ts"), JSON_TRANSFORM_CAPSULE)?;
    fs::write(project_path.join("README.md"), JSON_TRANSFORM_README)?;
    fs::write(project_path.join("package.json"), HELLO_WORLD_PACKAGE_JSON.replace("hello-world", "json-transform"))?;
    fs::write(project_path.join("tsconfig.json"), HELLO_WORLD_TSCONFIG)?;
    Ok(())
}

fn print_next_steps(project_name: &str, example_input: &str) {
    println!("\n📋 Next steps:");
    println!("   cd {}", project_name);
    println!("   npm install              # Install AssemblyScript");
    println!("   npm run build            # Compile to WASM");
    println!("   tenzik test build/capsule.wasm \"{}\"  # Test your capsule", example_input);
    println!("\n💡 Edit capsule.ts to implement your logic!");
    println!("📖 See README.md for more details");
}

fn print_next_steps_json(project_name: &str) {
    println!("\n📋 Next steps:");
    println!("   cd {}", project_name);
    println!("   npm install              # Install AssemblyScript");
    println!("   npm run build            # Compile to WASM");
    println!("   tenzik test build/capsule.wasm '{{\"data\":{{\"name\":\"Alice\"}},\"transform\":{{\"select\":[\"name\"]}}}}'");
    println!("\n💡 Edit capsule.ts to customize transformations!");
    println!("📖 See README.md for examples and use cases");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_hello_world() {
        let temp_dir = TempDir::new().unwrap();
        let project_name = "test-project";
        let project_path = temp_dir.path().join(project_name);

        let args = InitArgs {
            name: Some(project_path.to_string_lossy().to_string()),
            template: "hello-world".to_string(),
        };

        let result = execute_init_command(args);
        assert!(result.is_ok());

        // Verify files were created
        assert!(project_path.join("capsule.ts").exists());
        assert!(project_path.join("README.md").exists());
        assert!(project_path.join("package.json").exists());
        assert!(project_path.join("tsconfig.json").exists());
        assert!(project_path.join(".gitignore").exists());
        assert!(project_path.join("build").is_dir());
    }

    #[test]
    fn test_init_duplicate_directory() {
        let temp_dir = TempDir::new().unwrap();
        let project_name = "duplicate-test";
        let project_path = temp_dir.path().join(project_name);

        // Create directory first
        fs::create_dir_all(&project_path).unwrap();

        let args = InitArgs {
            name: Some(project_path.to_string_lossy().to_string()),
            template: "hello-world".to_string(),
        };

        let result = execute_init_command(args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }
}
