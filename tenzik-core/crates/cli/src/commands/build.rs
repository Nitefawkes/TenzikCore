use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum OptimizationLevel {
    None,
    Size,
    Speed,
    Aggressive,
}

impl OptimizationLevel {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "0" | "none" => Ok(OptimizationLevel::None),
            "s" | "size" => Ok(OptimizationLevel::Size),
            "1" | "2" | "speed" => Ok(OptimizationLevel::Speed),
            "3" | "z" | "aggressive" => Ok(OptimizationLevel::Aggressive),
            _ => bail!("Invalid optimization level: {}. Use 0/none, s/size, 1-2/speed, or 3/z/aggressive", s),
        }
    }
}

pub struct BuildArgs {
    pub output: Option<String>,
    pub optimize: Option<String>,
    pub watch: bool,
    pub verbose: bool,
}

#[derive(Debug)]
enum ProjectType {
    AssemblyScript,
    Rust,
}

pub fn execute_build_command(args: BuildArgs) -> Result<()> {
    println!("🔨 Building capsule...");

    // Detect project type
    let project_type = detect_project_type()?;
    println!("📦 Detected project type: {:?}", project_type);

    // Determine output path
    let output_path = args.output
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("build/capsule.wasm"));

    // Ensure build directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create build directory")?;
    }

    // Parse optimization level
    let opt_level = match args.optimize {
        Some(ref level) => OptimizationLevel::from_str(level)?,
        None => OptimizationLevel::Size, // Default to size optimization
    };

    // Build based on project type
    match project_type {
        ProjectType::AssemblyScript => {
            build_assemblyscript(&output_path, opt_level, args.verbose)?;
        }
        ProjectType::Rust => {
            build_rust(&output_path, opt_level, args.verbose)?;
        }
    }

    // Show build results
    show_build_results(&output_path)?;

    if args.watch {
        println!("\n👀 Watch mode not yet implemented. Use `cargo watch` or `nodemon` for now.");
    }

    Ok(())
}

fn detect_project_type() -> Result<ProjectType> {
    // Check for AssemblyScript
    if Path::new("capsule.ts").exists() || Path::new("src/capsule.ts").exists() {
        return Ok(ProjectType::AssemblyScript);
    }

    if Path::new("package.json").exists() {
        let package_json = fs::read_to_string("package.json")?;
        if package_json.contains("assemblyscript") {
            return Ok(ProjectType::AssemblyScript);
        }
    }

    // Check for Rust
    if Path::new("Cargo.toml").exists() {
        let cargo_toml = fs::read_to_string("Cargo.toml")?;
        if cargo_toml.contains("crate-type") && cargo_toml.contains("cdylib") {
            return Ok(ProjectType::Rust);
        }
    }

    if Path::new("src/lib.rs").exists() {
        return Ok(ProjectType::Rust);
    }

    bail!("Could not detect project type. Ensure you have either:\n  - capsule.ts (AssemblyScript)\n  - Cargo.toml with crate-type = [\"cdylib\"] (Rust)");
}

fn build_assemblyscript(output: &Path, opt_level: OptimizationLevel, verbose: bool) -> Result<()> {
    // Find source file
    let source = if Path::new("capsule.ts").exists() {
        "capsule.ts"
    } else if Path::new("src/capsule.ts").exists() {
        "src/capsule.ts"
    } else {
        bail!("Could not find capsule.ts in current directory or src/");
    };

    println!("  📝 Source: {}", source);
    println!("  🎯 Output: {}", output.display());

    // Check if asc is available
    let asc_check = Command::new("asc")
        .arg("--version")
        .output();

    if asc_check.is_err() {
        println!("\n⚠️  AssemblyScript compiler (asc) not found!");
        println!("   Install with: npm install -g assemblyscript");
        println!("   Or locally: npm install assemblyscript");
        bail!("AssemblyScript compiler not available");
    }

    // Build command
    let mut cmd = Command::new("asc");
    cmd.arg(source)
        .arg("-o")
        .arg(output)
        .arg("--runtime")
        .arg("stub");

    // Add optimization flags
    match opt_level {
        OptimizationLevel::None => {
            // No optimization
        }
        OptimizationLevel::Size => {
            cmd.arg("--optimize");
        }
        OptimizationLevel::Speed => {
            cmd.arg("--optimize");
        }
        OptimizationLevel::Aggressive => {
            cmd.arg("--optimize");
            cmd.arg("--shrinkLevel").arg("2");
            cmd.arg("--converge");
        }
    }

    if verbose {
        cmd.arg("--verbose");
    }

    println!("  ⚙️  Optimization: {:?}", opt_level);

    // Execute build
    let output_result = cmd.output()
        .context("Failed to execute asc compiler")?;

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        eprintln!("\n❌ Build failed:\n{}", stderr);
        bail!("AssemblyScript compilation failed");
    }

    if verbose {
        let stdout = String::from_utf8_lossy(&output_result.stdout);
        if !stdout.is_empty() {
            println!("{}", stdout);
        }
    }

    println!("  ✅ Compiled successfully!");

    Ok(())
}

fn build_rust(output: &Path, opt_level: OptimizationLevel, verbose: bool) -> Result<()> {
    println!("  📝 Source: src/lib.rs");
    println!("  🎯 Output: {}", output.display());

    // Check for wasm32 target
    let rustup_check = Command::new("rustup")
        .args(&["target", "list", "--installed"])
        .output();

    if let Ok(output_check) = rustup_check {
        let installed = String::from_utf8_lossy(&output_check.stdout);
        if !installed.contains("wasm32-unknown-unknown") {
            println!("\n⚠️  wasm32-unknown-unknown target not installed!");
            println!("   Installing now...");

            let install = Command::new("rustup")
                .args(&["target", "add", "wasm32-unknown-unknown"])
                .status()
                .context("Failed to install wasm32 target")?;

            if !install.success() {
                bail!("Failed to install wasm32-unknown-unknown target");
            }
        }
    }

    // Build with cargo
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown");

    // Add optimization flags
    match opt_level {
        OptimizationLevel::None => {
            // Debug build (default)
        }
        OptimizationLevel::Size | OptimizationLevel::Speed => {
            cmd.arg("--release");
        }
        OptimizationLevel::Aggressive => {
            cmd.arg("--release");
            // Note: Additional optimizations would go in Cargo.toml
        }
    }

    if verbose {
        cmd.arg("--verbose");
    }

    println!("  ⚙️  Optimization: {:?}", opt_level);

    // Execute build
    let status = cmd.status()
        .context("Failed to execute cargo")?;

    if !status.success() {
        bail!("Rust compilation failed");
    }

    // Find the built wasm file
    let wasm_path = if matches!(opt_level, OptimizationLevel::None) {
        "target/wasm32-unknown-unknown/debug/"
    } else {
        "target/wasm32-unknown-unknown/release/"
    };

    // Get crate name from Cargo.toml
    let cargo_toml = fs::read_to_string("Cargo.toml")
        .context("Failed to read Cargo.toml")?;

    let crate_name = cargo_toml
        .lines()
        .find(|line| line.trim_start().starts_with("name"))
        .and_then(|line| line.split('=').nth(1))
        .map(|s| s.trim().trim_matches('"'))
        .context("Could not find crate name in Cargo.toml")?;

    let source_wasm = PathBuf::from(wasm_path)
        .join(crate_name.replace('-', "_"))
        .with_extension("wasm");

    if !source_wasm.exists() {
        bail!("Built WASM file not found at: {}", source_wasm.display());
    }

    // Copy to output location
    fs::copy(&source_wasm, output)
        .with_context(|| format!("Failed to copy WASM to {}", output.display()))?;

    println!("  ✅ Compiled successfully!");

    Ok(())
}

fn show_build_results(output: &Path) -> Result<()> {
    let metadata = fs::metadata(output)
        .with_context(|| format!("Failed to read output file: {}", output.display()))?;

    let size_bytes = metadata.len();
    let size_kb = size_bytes as f64 / 1024.0;

    println!("\n📊 Build Results:");
    println!("  📦 Output: {}", output.display());
    println!("  📏 Size: {:.2} KB ({} bytes)", size_kb, size_bytes);

    // Provide size guidance
    if size_kb > 100.0 {
        println!("  ⚠️  Capsule is larger than 100 KB. Consider:");
        println!("     - Using --optimize aggressive");
        println!("     - Removing unused code");
        println!("     - Using wasm-opt for further optimization");
    } else if size_kb > 50.0 {
        println!("  💡 Capsule size is acceptable but could be smaller");
    } else {
        println!("  ✅ Capsule size is excellent!");
    }

    // Validate the WASM
    println!("\n🔍 Validating WASM...");
    match validate_wasm_basic(output) {
        Ok(_) => println!("  ✅ WASM structure is valid"),
        Err(e) => println!("  ⚠️  Validation warning: {}", e),
    }

    println!("\n💡 Next steps:");
    println!("  tenzik test {} \"your input\"", output.display());
    println!("  tenzik validate {}", output.display());

    Ok(())
}

fn validate_wasm_basic(path: &Path) -> Result<()> {
    let bytes = fs::read(path)?;

    // Check WASM magic number
    if bytes.len() < 4 {
        bail!("File too small to be valid WASM");
    }

    if &bytes[0..4] != b"\0asm" {
        bail!("Invalid WASM magic number");
    }

    // Check version
    if bytes.len() < 8 {
        bail!("Missing WASM version");
    }

    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != 1 {
        bail!("Unsupported WASM version: {}", version);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_level_parsing() {
        assert!(matches!(OptimizationLevel::from_str("0").unwrap(), OptimizationLevel::None));
        assert!(matches!(OptimizationLevel::from_str("s").unwrap(), OptimizationLevel::Size));
        assert!(matches!(OptimizationLevel::from_str("speed").unwrap(), OptimizationLevel::Speed));
        assert!(matches!(OptimizationLevel::from_str("3").unwrap(), OptimizationLevel::Aggressive));
        assert!(OptimizationLevel::from_str("invalid").is_err());
    }

    #[test]
    fn test_wasm_validation() {
        // Valid WASM header
        let valid_wasm = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), &valid_wasm).unwrap();
        assert!(validate_wasm_basic(temp.path()).is_ok());

        // Invalid magic number
        let invalid_wasm = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00];
        let temp2 = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp2.path(), &invalid_wasm).unwrap();
        assert!(validate_wasm_basic(temp2.path()).is_err());
    }
}
