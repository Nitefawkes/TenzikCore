use clap::{Args, Parser, Subcommand};
use anyhow::Result;

mod commands;
use commands::{TestArgs, execute_test_command, validate_capsule_file, NodeArgs, execute_node_command, validate_db_path, parse_peer_address};

#[derive(Parser)]
#[command(name = "tenzik")]
#[command(about = "Tenzik Core - Verifiable edge compute with WASM capsules")]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Tenzik project
    Init(InitArgs),
    /// Test a capsule locally  
    Test(TestCommandArgs),
    /// Validate a capsule without executing
    Validate(ValidateArgs),
    /// Start a Tenzik node
    Node(NodeArgs),
    /// Verify an execution receipt
    Receipt(ReceiptArgs),
}

#[derive(Args)]
pub struct InitArgs {
    /// Project directory name
    pub name: Option<String>,
    /// Template to use
    #[arg(short, long, default_value = "hello-world")]
    pub template: String,
}

#[derive(Args)]
pub struct TestCommandArgs {
    /// Path to WASM capsule
    pub capsule: String,
    /// Input JSON string
    pub input: String,
    /// Show execution metrics
    #[arg(long)]
    pub metrics: bool,
    /// Show full receipt JSON
    #[arg(long)]
    pub show_receipt: bool,
    /// Custom resource limits (JSON format)
    #[arg(long)]
    pub limits: Option<String>,
}

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to WASM capsule
    pub capsule: String,
}

#[derive(Args)]  
pub struct NodeArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "9000")]
    pub port: u16,
    /// Peer address to connect to
    #[arg(short = 'p', long)]
    pub peer: Option<String>,
    /// Local database path
    #[arg(long, default_value = ".tenzik")]
    pub db: String,
    /// Node name
    #[arg(short, long)]
    pub name: Option<String>,
}

#[derive(Args)]
pub struct ReceiptArgs {
    #[command(subcommand)]
    pub command: ReceiptCommands,
}

#[derive(Subcommand)]
pub enum ReceiptCommands {
    /// Verify a receipt signature
    Verify { receipt_id: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init(args) => {
            println!("🚀 Initializing Tenzik project: {:?}", args.name);
            // TODO: Implement project initialization
            Ok(())
        }
        Commands::Test(args) => {
            let test_args = TestArgs {
                capsule: args.capsule,
                input: args.input,
                metrics: args.metrics,
                show_receipt: args.show_receipt,
                limits: args.limits,
            };
            execute_test_command(test_args).await
        }
        Commands::Validate(args) => {
            validate_capsule_file(&args.capsule)
        }
        Commands::Node(args) => {
            // Validate database path
            validate_db_path(&args.db)?;
            
            // Validate peer address if provided
            if let Some(peer_str) = &args.peer {
                parse_peer_address(peer_str)?;
            }
            
            let node_args = commands::NodeArgs {
                port: args.port,
                peer: args.peer,
                db: args.db,
                name: args.name,
            };
            
            execute_node_command(node_args).await
        }
        Commands::Receipt(args) => {
            match args.command {
                ReceiptCommands::Verify { receipt_id } => {
                    println!("🔍 Verifying receipt: {}", receipt_id);
                    // TODO: Implement receipt verification
                    Ok(())
                }
            }
        }
    }
}
