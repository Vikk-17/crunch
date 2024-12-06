//! Turbocharge your Rust workflow.
//!
//! crunch seamlessly integrates cutting-edge hardware into your local development environment.

use clap::{command, Parser};
use env_logger;
use log::{debug, error, info};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    process::{exit, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
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

    /// Path or directory to exclude from the remote server transfer.
    /// Specify multiple entries using delimiter ','.
    ///
    /// By default the `target` and `.git` directories are excluded.
    ///
    /// Example: `--exclude "target,.git,cat.png,*.lock,mocks/**/*.db"`
    #[arg(
        long = "exclude",
        required = false,
        value_delimiter = ',',
        default_value = "target,.git"
    )]
    exclude: Vec<String>,

    /// A command to execute on the machine after the cargo command has finished executing.
    ///
    /// Example: `--post-cargo "cd target/release && profile my-binary"`
    #[arg(long = "post-cargo", required = false)]
    post_cargo: Option<String>,

    /// Path or directory to sync back from the remote server after all other work has been done.
    /// Each entry should be in the format `source:destination`. Specify multiple entries using delimiter ','.
    ///
    /// Example: `--copy-back "./target/release/cuter-cat.png:.,*.bin:~/my-bins"`
    #[arg(long = "copy-back", required = false, value_delimiter = ',')]
    copy_back: Vec<String>,

    /// The cargo command to execute
    ///
    /// Example: `build --release`
    #[arg(required = true, num_args = 1..)]
    command: Vec<String>,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();
    debug!("{:?}", &args);

    let copy_back_pairs: Vec<(String, String)> = args
        .copy_back
        .into_iter()
        .filter_map(|entry| {
            let mut parts = entry.splitn(2, ':');
            match (parts.next(), parts.next()) {
                (Some(source), Some(dest)) => Some((source.to_string(), dest.to_string())),
                _ => {
                    panic!("Invalid format for --copy-back entry: {}", entry);
                }
            }
        })
        .collect();

    let manifest_path = extract_manifest_path(&args.command).unwrap_or("Cargo.toml".to_string());
    let mut metadata_cmd = cargo_metadata::MetadataCommand::new();
    metadata_cmd
        .manifest_path(manifest_path)
        .no_deps()
        .other_options(&["--offline".to_string()]);
    let project_metadata = metadata_cmd.exec().unwrap();
    let project_dir = project_metadata.workspace_root;

    let remote = Remote {
        name: "crunch".to_string(),
        host: "crunch".to_string(),
        ssh_port: 22,
        temp_dir: "~/crunch-builds".to_string(),
        env: "~/.profile".to_string(),
    };

    let build_server = remote.host;

    let mut hasher = DefaultHasher::new();
    project_dir.hash(&mut hasher);
    let build_path = format!("{}/{}/", remote.temp_dir, hasher.finish());

    info!("Transferring sources to remote: {}", build_path);
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

    args.exclude.iter().for_each(|exclude| {
        rsync_to.arg("--exclude").arg(exclude);
    });

    rsync_to
        .arg("--rsync-path")
        .arg(format!("mkdir -p {} && rsync", build_path))
        .arg(format!("{}/", project_dir.to_string()))
        .arg(format!("{}:{}", build_server, build_path))
        .env("LC_ALL", "C.UTF-8")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap_or_else(|e| {
            error!("Failed to transfer project to build server (error: {})", e);
            exit(-4);
        });

    let build_command = format!(
        "export CC=gcc; export CXX=g++; source {}; cd {}; {} cargo {}",
        remote.env,
        build_path,
        args.build_env,
        args.command.join(" "),
    );

    // Add the post_cargo command to the build_command, if it exists
    let command = if let Some(post_cargo) = args.post_cargo {
        format!(
            "{} && echo Executing post-cargo command && {}",
            build_command, post_cargo
        )
    } else {
        build_command
    };
    let _output = Command::new("ssh")
        .env("LC_ALL", "C.UTF-8")
        .args(&["-p", &remote.ssh_port.to_string()])
        .arg("-t")
        .arg(&build_server)
        .arg(command)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap_or_else(|e| {
            error!("Failed to run cargo command remotely (error: {})", e);
            exit(-5);
        });

    if !copy_back_pairs.is_empty() {
        info!("Transferring artifacts back to the local machine.");

        let errors = Arc::new(Mutex::new(Vec::new()));
        let threads: Vec<_> = copy_back_pairs
            .into_iter()
            .map(|(remote_source, local_dest)| {
                let errors = Arc::clone(&errors);
                let build_server = build_server.clone();
                let build_path = build_path.clone();
                thread::spawn(move || {
                    let mut rsync_back = Command::new("rsync");
                    rsync_back
                        .arg("-a")
                        .arg("--compress")
                        .arg("-e")
                        .arg(format!("ssh -p {}", remote.ssh_port))
                        .arg("--info=progress2")
                        .arg(format!(
                            "{}:{}/{}",
                            &build_server, build_path, remote_source
                        ))
                        .arg(format!("{}/", local_dest))
                        .env("LC_ALL", "C.UTF-8")
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .stdin(Stdio::inherit());

                    let output = rsync_back.output();

                    match output {
                        Ok(result) if result.status.success() => {
                            info!(
                                "Successfully transferred '{}' to '{}'",
                                remote_source, local_dest
                            );
                        }
                        Ok(result) => {
                            let message = format!(
                                "Rsync failed for '{}' to '{}' with exit code: {}",
                                remote_source, local_dest, result.status
                            );
                            error!("{}", message);
                            errors.lock().unwrap().push(message);
                        }
                        Err(e) => {
                            let message = format!(
                                "Failed to transfer '{}' to '{}' (error: {})",
                                remote_source, local_dest, e
                            );
                            error!("{}", message);
                            errors.lock().unwrap().push(message);
                        }
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().unwrap();
        }

        let errors = errors.lock().unwrap();
        if !errors.is_empty() {
            for error in errors.iter() {
                eprintln!("{}", error);
            }
            exit(-6);
        }
    }
}

fn extract_manifest_path(args: &[String]) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--manifest-path" {
            return args.next().cloned();
        } else if arg.starts_with("--manifest-path=") {
            return Some(arg.splitn(2, '=').nth(1).unwrap().to_string());
        }
    }
    None
}

#[test]
fn extract_manifest_path_works() {
    // Test next arg
    let args = vec![
        "build".to_string(),
        "--release".to_string(),
        "--manifest-path".to_string(),
        "Cargo.toml".to_string(),
    ];
    assert_eq!(extract_manifest_path(&args), Some("Cargo.toml".to_string()));

    // Test equals
    let args = vec![
        "build".to_string(),
        "--release".to_string(),
        "--manifest-path=Cargo.toml".to_string(),
    ];
    assert_eq!(extract_manifest_path(&args), Some("Cargo.toml".to_string()));

    // Test none
    let args = vec!["build".to_string(), "--release".to_string()];
    assert_eq!(extract_manifest_path(&args), None);
}
