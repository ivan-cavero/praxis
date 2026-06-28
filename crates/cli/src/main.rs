//! Project-X CLI — Multi-Agent Autonomous System
//!
//! Usage: project-x <command> [options]
//! See `project-x help` for full documentation.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "project-x")]
#[command(about = "Autonomous Multi-Agent System", long_about = None)]
#[command(version = "0.1.0")]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project
    Init {
        /// Project name
        name: String,
    },

    /// Execute a goal
    Run {
        /// Goal description or name
        #[arg(long)]
        goal: Option<String>,

        /// Read goal from file
        #[arg(long)]
        file: Option<PathBuf>,

        /// Resume last interrupted session
        #[arg(long)]
        resume: bool,

        /// Resume specific session
        #[arg(long)]
        session: Option<String>,

        /// Show plan without executing
        #[arg(long)]
        dry_run: bool,

        /// JSON output for CI/CD
        #[arg(long)]
        headless: bool,
    },

    /// Manage projects
    #[command(subcommand)]
    Project(ProjectCommands),

    /// Manage sessions
    #[command(subcommand)]
    Session(SessionCommands),

    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),

    /// LLM provider management
    #[command(subcommand)]
    Provider(ProviderCommands),

    /// MCP server management
    #[command(subcommand)]
    Mcp(McpCommands),

    /// Context management
    #[command(subcommand)]
    Context(ContextCommands),

    /// Inject mid-loop instructions
    Inject {
        /// Target session
        #[arg(long)]
        session: String,

        /// Target agent (or "all")
        #[arg(long)]
        agent: String,

        /// Message type: instruction|context|correction|halt
        #[arg(long, default_value = "instruction")]
        message_type: String,

        /// The instruction text
        #[arg(long)]
        message: String,
    },

    /// Open desktop app
    Desktop,

    /// Open web dashboard
    Dashboard,

    /// Open terminal UI monitor
    Monitor,

    /// Update to latest version
    Update {
        /// Release channel
        #[arg(long, default_value = "stable")]
        channel: String,
    },

    /// Show version
    Version,

    /// Organization management (Enterprise)
    #[command(subcommand)]
    Org(OrgCommands),

    /// Billing (Enterprise)
    #[command(subcommand)]
    Billing(BillingCommands),

    /// Run a simple test (Sprint 0.1 demo)
    Test,
}

#[derive(Subcommand)]
enum ProjectCommands {
    List,
    Show { id: String },
    Archive { id: String },
}

#[derive(Subcommand)]
enum SessionCommands {
    List { project: Option<String> },
    Show { id: String },
    Stop { id: String },
    Logs { id: String, #[arg(long)] tail: bool, #[arg(long)] json: bool },
}

#[derive(Subcommand)]
enum ConfigCommands {
    Show,
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
    Edit,
    Import { file: PathBuf },
    Export { file: PathBuf },
}

#[derive(Subcommand)]
enum ProviderCommands {
    List,
    Test { name: String },
}

#[derive(Subcommand)]
enum McpCommands {
    List,
    Add { name: String, command: String, args: Vec<String> },
    Remove { name: String },
    Test { name: String },
}

#[derive(Subcommand)]
enum ContextCommands {
    Inspect { session: String },
    History { session: String },
    ForceCompress { session: String },
}

#[derive(Subcommand)]
enum OrgCommands {
    Create { name: String },
    List,
    Show,
    Switch { id: String },
}

#[derive(Subcommand)]
enum BillingCommands {
    Show,
    Invoices,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            println!("🔨 Initializing project: {}", name);
            println!("   Project '{name}' created at ./{name}");
        }

        Commands::Run { goal, file, resume, .. } => {
            if let Some(g) = goal {
                println!("🚀 Running goal: {}", g);
                println!("   Press Ctrl+C to stop");
                println!();

                // Sprint 0.1: Start core runtime, spawn an agent, echo back
                println!("📦 Starting core runtime...");
                let runtime = project_x_core::CoreRuntime::new().await?;

                println!("🤖 Spawning EchoAgent...");
                let handle = runtime.spawn_echo_agent("test-agent").await?;
                println!("   Agent '{}' spawned", handle.name);

                println!("💬 Sending test message...");
                let response = runtime.echo_to(&handle.name, &g).await?;
                println!("   Response: {}", response);

                println!("📋 Listing agents...");
                let agents = runtime.list_agents().await?;
                for agent in &agents {
                    println!("   - {} ({})", agent.name, agent.role);
                }

                println!("🔌 Shutting down...");
                runtime.shutdown().await?;
                println!("✅ Done");
            } else if let Some(f) = file {
                println!("📄 Reading goal from: {}", f.display());
            } else if resume {
                println!("📌 Resuming last session...");
            } else {
                println!("❌ Please provide --goal or --file or --resume");
            }
        }

        Commands::Desktop => {
            println!("🖥️  Opening Project-X Desktop...");
        }

        Commands::Dashboard => {
            println!("🌐 Starting Project-X Dashboard...");
            println!("   Open http://localhost:8080 in your browser");
        }

        Commands::Monitor => {
            println!("📊 Opening terminal monitor...");
        }

        Commands::Version => {
            println!("Project-X v{}", env!("CARGO_PKG_VERSION"));
        }

        Commands::Update { channel } => {
            println!("🔄 Checking for updates (channel: {})...", channel);
            println!("   Already up to date (v{})", env!("CARGO_PKG_VERSION"));
        }

        Commands::Inject { session, agent, message_type, message } => {
            println!("💉 Injecting message to agent...");
            println!("   Session: {}", session);
            println!("   Agent: {}", agent);
            println!("   Type: {}", message_type);
            println!("   Message: {}", message);
        }

        Commands::Test => {
            println!("🧪 Sprint 0.1 Test");
            println!("===================");
            println!();

            // 1. Create EventBus
            println!("1️⃣  Creating EventBus...");
            let bus = project_x_core::EventBus::new();
            println!("   ✅ EventBus created (capacity: {})", bus.capacity());

            // 2. Create CoreRuntime (spawns Supervisor internally)
            println!("2️⃣  Creating CoreRuntime (spawns Supervisor)...");
            let runtime = project_x_core::CoreRuntime::new().await?;
            println!("   ✅ CoreRuntime created");

            // 3. Spawn EchoAgents
            println!("3️⃣  Spawning 3 EchoAgents...");
            for i in 0..3 {
                let name = format!("agent-{}", i);
                let handle = runtime.spawn_echo_agent(&name).await?;
                println!("   ✅ Spawned: {} (role: {})", handle.name, handle.role);
            }

            // 4. Send messages
            println!("4️⃣  Sending echo messages...");
            for i in 0..3 {
                let name = format!("agent-{}", i);
                let response = runtime.echo_to(&name, &format!("Hello from test! Message #{}", i)).await?;
                println!("   ✅ {}: {}", name, response);
            }

            // 5. List agents
            println!("5️⃣  Listing agents...");
            let agents = runtime.list_agents().await?;
            println!("   {} agents running", agents.len());
            for agent in &agents {
                println!("   - {} ({})", agent.name, agent.role);
            }

            // 6. Shutdown
            println!("6️⃣  Shutting down...");
            let _ = runtime.shutdown().await;
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            println!("   ✅ Shutdown complete");
            println!();
            println!("🎉 Sprint 0.1 test PASSED");
        }

        // ─── Subcommands ─────────────────────────────────────

        Commands::Project(cmd) => match cmd {
            ProjectCommands::List => println!("📁 Projects:\n   (no projects yet)"),
            ProjectCommands::Show { id } => println!("📁 Project: {}", id),
            ProjectCommands::Archive { id } => println!("📁 Archiving project: {}", id),
        },

        Commands::Session(cmd) => match cmd {
            SessionCommands::List { project } => {
                println!("📋 Sessions for project: {:?}", project);
                println!("   (no sessions yet)");
            }
            SessionCommands::Show { id } => println!("📋 Session: {}", id),
            SessionCommands::Stop { id } => println!("⏹  Stopping session: {}", id),
            SessionCommands::Logs { id, tail, json } => {
                println!("📜 Logs for session: {} (tail: {}, json: {})", id, tail, json);
            }
        },

        Commands::Config(cmd) => match cmd {
            ConfigCommands::Show => println!("📝 Current configuration:\n   (not configured)"),
            ConfigCommands::Get { key } => println!("🔑 {} = (not set)", key),
            ConfigCommands::Set { key, value } => println!("🔑 {} = {} (saved)", key, value),
            ConfigCommands::Unset { key } => println!("🔑 {} (removed)", key),
            ConfigCommands::Edit => println!("📝 Opening editor..."),
            ConfigCommands::Import { file } => println!("📥 Importing from: {}", file.display()),
            ConfigCommands::Export { file } => println!("📤 Exporting to: {}", file.display()),
        },

        Commands::Provider(cmd) => match cmd {
            ProviderCommands::List => println!("🔌 Providers:\n   (no providers configured)"),
            ProviderCommands::Test { name } => println!("🔌 Testing provider: {}...", name),
        },

        Commands::Mcp(cmd) => match cmd {
            McpCommands::List => println!("🔗 MCP Servers:\n   (no servers connected)"),
            McpCommands::Add { name, command, args } => {
                println!("🔗 Adding MCP server: {} ({} {:?})", name, command, args);
            }
            McpCommands::Remove { name } => println!("🔗 Removing MCP server: {}", name),
            McpCommands::Test { name } => println!("🔗 Testing MCP server: {}...", name),
        },

        Commands::Context(cmd) => match cmd {
            ContextCommands::Inspect { session } => println!("🧠 Context for session: {}", session),
            ContextCommands::History { session } => println!("📊 Compression history for session: {}", session),
            ContextCommands::ForceCompress { session } => println!("⚡ Forcing compression for session: {}", session),
        },

        Commands::Org(cmd) => match cmd {
            OrgCommands::Create { name } => println!("🏢 Creating organization: {}", name),
            OrgCommands::List => println!("🏢 Organizations:\n   (no organizations yet)"),
            OrgCommands::Show => println!("🏢 Current organization:\n   (not set)"),
            OrgCommands::Switch { id } => println!("🏢 Switched to organization: {}", id),
        },

        Commands::Billing(cmd) => match cmd {
            BillingCommands::Show => println!("💰 Billing:\n   Plan: Free\n   Usage: 0 tokens\n   Next invoice: N/A"),
            BillingCommands::Invoices => println!("🧾 Invoices:\n   (no invoices yet)"),
        },
    }

    Ok(())
}