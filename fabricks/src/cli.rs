//! CLI argument definitions using clap.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Fabricks - Declarative WASM Orchestration Platform.
///
/// Build, deploy, and manage WebAssembly microservices with a familiar,
/// container-like workflow.
#[derive(Parser, Debug)]
#[command(name = "fabricks")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Build a WASM module from a Fabrickfile.
    ///
    /// Compiles the source code and packages it as an OCI artifact.
    Build(BuildArgs),

    /// Run a WASM module locally.
    ///
    /// Executes the module with the specified capabilities.
    Run(RunArgs),

    /// Push a module to an OCI registry.
    ///
    /// Uploads the built module to a container registry.
    Push(PushArgs),

    /// Pull a module from an OCI registry.
    ///
    /// Downloads a module from a container registry.
    Pull(PullArgs),

    /// Inspect a WASM module.
    ///
    /// Display metadata and capabilities of a module.
    Inspect(InspectArgs),

    /// Validate configuration files.
    ///
    /// Checks Fabrickfile or fabricks-mortar.toml for syntax errors
    /// and validates all fields according to the specification.
    Validate(ValidateArgs),

    /// Log in to an OCI registry.
    ///
    /// Saves credentials for pushing and pulling modules.
    Login(LoginArgs),

    /// Log out from an OCI registry.
    ///
    /// Removes saved credentials for a registry.
    Logout(LogoutArgs),

    /// Show version information.
    Version,

    /// Daemon management commands.
    ///
    /// Interact with the fabricksd daemon.
    Daemon(DaemonArgs),

    /// Service management commands.
    ///
    /// Manage services running on the daemon.
    Service(ServiceArgs),

    /// Mortar project commands.
    ///
    /// Manage multi-service deployments.
    Mortar(MortarArgs),
}

/// Arguments for the build command.
#[derive(Parser, Debug)]
pub struct BuildArgs {
    /// Path to the Fabrickfile or directory containing one.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Tag for the built module (e.g., "my-module:1.0.0").
    ///
    /// If not specified, uses the name and version from the Fabrickfile.
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Skip running the build command (use pre-built WASM).
    #[arg(long)]
    pub no_build: bool,

    /// Output format for build results.
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Arguments for the run command.
#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Module reference (local path, tag, or registry reference).
    ///
    /// Examples:
    /// - ./module.wasm
    /// - my-module:1.0.0
    /// - ghcr.io/user/module:latest
    pub module: String,

    /// Arguments to pass to the WASM module.
    #[arg(last = true)]
    pub args: Vec<String>,

    /// Override environment variables to pass to the module.
    ///
    /// Format: NAME=VALUE
    #[arg(short, long = "env")]
    pub envs: Vec<String>,

    /// Don't enforce capability restrictions.
    #[arg(long)]
    pub no_capabilities: bool,
}

/// Arguments for the push command.
#[derive(Parser, Debug)]
pub struct PushArgs {
    /// Module reference to push (local tag).
    ///
    /// Example: my-module:1.0.0
    pub source: String,

    /// Registry reference to push to.
    ///
    /// Example: ghcr.io/user/my-module:1.0.0
    pub destination: String,

    /// Accept invalid TLS certificates (for testing).
    #[arg(long)]
    pub insecure: bool,
}

/// Arguments for the pull command.
#[derive(Parser, Debug)]
pub struct PullArgs {
    /// Registry reference to pull from.
    ///
    /// Example: ghcr.io/user/my-module:1.0.0
    pub reference: String,

    /// Local tag to save as.
    ///
    /// If not specified, uses the reference as the tag.
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Accept invalid TLS certificates (for testing).
    #[arg(long)]
    pub insecure: bool,
}

/// Arguments for the inspect command.
#[derive(Parser, Debug)]
pub struct InspectArgs {
    /// Module reference to inspect (local path, tag, or registry reference).
    pub module: String,

    /// Output format for inspection results.
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Arguments for the login command.
#[derive(Parser, Debug)]
pub struct LoginArgs {
    /// Registry to authenticate with.
    ///
    /// Example: ghcr.io
    pub registry: String,

    /// Username for authentication.
    ///
    /// If not provided, will prompt interactively.
    #[arg(short, long)]
    pub username: Option<String>,

    /// Password for authentication.
    ///
    /// If not provided, will prompt interactively.
    /// Consider using --password-stdin for security.
    #[arg(short, long)]
    pub password: Option<String>,

    /// Read password from stdin.
    #[arg(long)]
    pub password_stdin: bool,
}

/// Arguments for the logout command.
#[derive(Parser, Debug)]
pub struct LogoutArgs {
    /// Registry to log out from.
    ///
    /// Example: ghcr.io
    pub registry: String,
}

/// Arguments for the validate command.
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Path to the file or directory to validate.
    ///
    /// If a directory is provided, looks for Fabrickfile or
    /// fabricks-mortar.toml based on the --type flag.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Type of file to validate.
    ///
    /// If not specified, attempts to auto-detect based on filename.
    #[arg(short = 't', long = "type", value_enum)]
    pub file_type: Option<FileType>,

    /// Output format for validation results.
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Type of configuration file.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FileType {
    /// Fabrickfile (single service definition).
    Fabrickfile,
    /// fabricks-mortar.toml (multi-service composition).
    Mortar,
}

/// Output format for command results.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output.
    #[default]
    Text,
    /// JSON output for programmatic consumption.
    Json,
}

/// Arguments for daemon commands.
#[derive(Args, Debug)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonCommands,
}

/// Daemon subcommands.
#[derive(Subcommand, Debug)]
pub enum DaemonCommands {
    /// Show daemon information.
    ///
    /// Displays version, uptime, and configuration of the running daemon.
    Info {
        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Custom socket path.
        ///
        /// Override the default Unix socket path.
        #[arg(long)]
        socket: Option<PathBuf>,
    },
}

/// Arguments for service commands.
#[derive(Args, Debug)]
pub struct ServiceArgs {
    /// Custom socket path.
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: ServiceCommands,
}

/// Service subcommands.
#[derive(Subcommand, Debug)]
pub enum ServiceCommands {
    /// List all running services.
    #[command(name = "ls", alias = "list")]
    List {
        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Deploy and start a service from a Fabrickfile.
    ///
    /// Creates the service and starts it immediately.
    Run {
        /// Path to the Fabrickfile or directory containing one.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Path to pre-built WASM module (skips build step).
        #[arg(long)]
        wasm: Option<PathBuf>,

        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Get detailed information about a service.
    #[command(name = "inspect", alias = "get")]
    Inspect {
        /// Service ID or name.
        id: String,

        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Start a service.
    Start {
        /// Service ID.
        id: String,
    },

    /// Stop a service.
    Stop {
        /// Service ID.
        id: String,
    },

    /// Scale a service to a target number of replicas.
    Scale {
        /// Service ID.
        id: String,

        /// Target number of replicas.
        replicas: usize,
    },

    /// Remove (delete) a stopped service.
    #[command(name = "rm", alias = "remove")]
    Remove {
        /// Service ID.
        id: String,

        /// Force removal (stop first if running).
        #[arg(short, long)]
        force: bool,
    },
}

/// Arguments for mortar commands.
#[derive(Args, Debug)]
pub struct MortarArgs {
    /// Custom socket path.
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: MortarCommands,
}

/// Mortar project subcommands.
#[derive(Subcommand, Debug)]
pub enum MortarCommands {
    /// Deploy a mortar project.
    ///
    /// Starts all services defined in fabricks-mortar.toml.
    Up {
        /// Path to fabricks-mortar.toml or directory containing it.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Tear down a mortar project.
    ///
    /// Stops and removes all services in the project.
    Down {
        /// Project name.
        project: String,
    },

    /// Show status of services in a project.
    #[command(name = "ps", alias = "status")]
    Status {
        /// Project name.
        project: String,

        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// List all mortar projects.
    #[command(name = "ls", alias = "list")]
    List {
        /// Output format.
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
}
