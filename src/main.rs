//! Turbocharge your Rust workflow.
//!
//! crunch seamlessly integrates cutting-edge hardware into your local development environment.

use clap::{command, Parser};
use log::{debug, error, info};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    process::{exit, Command, Stdio},
};

#[derive(Debug, Clone)]
pub struct Remote {
    pub name: String,
    pub host: String,
    pub ssh_port: u16,
    pub temp_dir: String,
    pub env: String,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about,
    trailing_var_arg = true,
    after_long_help = "EXAMPLES:\n    crunch -e RUST_LOG=debug check --all-features --all-targets\n    crunch test -- --nocapture"
)]
struct Args {
    /// Set remote environment variables. RUST_BACKTRACE, CC, LIB, etc.
    #[arg(
        short = 'e',
        long,
        required = false,
        default_value = "RUST_BACKTRACE=1"
    )]
    build_env: String,

    /// Whether to include hidden files to the remote server transfer
    #[arg(long, required = false)]
    hidden: bool,

    /// Path or directory to exclude from the remote server transfer.
    /// Specify multiple using delimiter ','. Supports regex glob patterns.
    ///
    /// Example: `--exclude "cat.png,*.lock,mocks/**/*.db"`
    #[arg(long = "exclude", required = false, value_delimiter = ',')]
    exclude: Vec<String>,

    /// The cargo command to execute
    #[arg(required = true, num_args = 1..)]
    command: Vec<String>,
}

fn main() {
    let args = Args::parse();

    debug!("{:?}", &args);

    let mut metadata_cmd = cargo_metadata::MetadataCommand::new();
    metadata_cmd.manifest_path("Cargo.toml").no_deps();

    let project_metadata = metadata_cmd.exec().unwrap();
    let project_dir = project_metadata.workspace_root;
    info!("Project dir: {:?}", project_dir);

    let remote = Remote {
        name: "crunch".to_string(),
        host: "crunch-ax102".to_string(),
        ssh_port: 22,
        temp_dir: "/tmp".to_string(),
        env: "~/.profile".to_string(),
    };

    let build_server = remote.host;

    let mut hasher = DefaultHasher::new();
    project_dir.hash(&mut hasher);
    let build_path = format!("{}/{}/", remote.temp_dir, hasher.finish());

    info!("Transferring sources to build server.");
    let mut rsync_to = Command::new("rsync");
    rsync_to
        .arg("-a".to_owned())
        .arg("--delete")
        .arg("--compress")
        .arg("-e")
        .arg(format!("ssh -p {}", remote.ssh_port))
        .arg("--info=progress2")
        .arg("--exclude")
        .arg("target");

    if !args.hidden {
        rsync_to.arg("--exclude").arg(".*");
    }

    args.exclude.iter().for_each(|exclude| {
        rsync_to.arg("--exclude").arg(exclude);
    });

    rsync_to
        .arg("--rsync-path")
        .arg("mkdir -p remote-builds && rsync")
        .arg(format!("{}/", project_dir.to_string_lossy()))
        .arg(format!("{}:{}", build_server, build_path))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap_or_else(|e| {
            error!("Failed to transfer project to build server (error: {})", e);
            exit(-4);
        });
    info!("Build ENV: {:?}", args.build_env);
    info!("Environment profile: {:?}", remote.env);
    info!("Build path: {:?}", build_path);

    let build_command = format!(
        "export CC=gcc; export CXX=g++; source {}; cd {}; {} cargo {}",
        remote.env,
        build_path,
        args.build_env,
        args.command.join(" "),
    );

    info!("Starting build process.");
    let _output = Command::new("ssh")
        .args(&["-p", &remote.ssh_port.to_string()])
        .arg("-t")
        .arg(&build_server)
        .arg(build_command)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap_or_else(|e| {
            error!("Failed to run cargo command remotely (error: {})", e);
            exit(-5);
        });
}
