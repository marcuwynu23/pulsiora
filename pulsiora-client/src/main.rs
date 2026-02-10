use clap::{Parser, Subcommand};
use pulsiora_core::PipelineExecution;
use pulsiora_parser::parse_pulsefile;
use pulsiora_runner::PipelineExecutor;
use reqwest::Client;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(name = "pulse")]
#[command(about = "Pulsiora CI/CD CLI client", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Server URL
    #[arg(long, default_value = "http://localhost:3000")]
    server: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Check server health
    Health,

    /// Generate Pulsefile template
    Init,

    /// Repository management
    #[command(subcommand)]
    Repo(RepoCommands),

    /// Pipeline management
    #[command(subcommand)]
    Pipeline(PipelineCommands),

    /// Get pipeline execution details (deprecated: use pipeline logs)
    Status {
        /// Execution ID
        id: String,
    },

    /// List all pipeline executions
    List,
    
    /// Manually execute a Pulsefile
    Run {
        /// Path to Pulsefile
        #[arg(short, long, default_value = "Pulsefile")]
        pulsefile: String,
        
        /// Repository URL (for logging purposes)
        #[arg(short, long, default_value = "local/repo")]
        repo_url: String,
        
        /// Branch name (for logging purposes)
        #[arg(short, long, default_value = "main")]
        branch: String,
    },
}

#[derive(Subcommand)]
enum RepoCommands {
    /// Register repository and upload Pulsefile
    Add {
        /// Repository URL (e.g., https://github.com/owner/repo)
        repo_url: String,
        
        /// Path to Pulsefile (defaults to ./Pulsefile)
        #[arg(short, long, default_value = "Pulsefile")]
        pulsefile: String,
        
        /// Repository type (github, local, or other SCM)
        #[arg(short, long, default_value = "github")]
        repo_type: String,
    },

    /// Unregister repository
    Remove {
        /// Repository URL (e.g., https://github.com/owner/repo)
        repo_url: String,
    },
}

#[derive(Subcommand)]
enum PipelineCommands {
    /// Check recent pipeline runs for a repository
    Status {
        /// Repository (e.g., owner/repo or full URL)
        repo: String,
        
        /// Number of runs to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Fetch logs for a specific pipeline run
    Logs {
        /// Repository (e.g., owner/repo or full URL)
        repo: String,
        
        /// Run ID (execution ID)
        run_id: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Health => {
            let url = format!("{}/health", cli.server);
            let response = client.get(&url).send().await?;
            if response.status().is_success() {
                println!("Server is healthy");
                process::exit(0);
            } else {
                eprintln!("Server is unhealthy");
                process::exit(1);
            }
        }
        Commands::Init => {
            generate_pulsefile_template()?;
        }
        Commands::Repo(cmd) => match cmd {
            RepoCommands::Add { repo_url, pulsefile, repo_type } => {
                register_repo(&client, &cli.server, &repo_url, &pulsefile, &repo_type).await?;
            }
            RepoCommands::Remove { repo_url } => {
                unregister_repo(&client, &cli.server, &repo_url).await?;
            }
        },
        Commands::Pipeline(cmd) => match cmd {
            PipelineCommands::Status { repo, limit } => {
                get_pipeline_status(&client, &cli.server, &repo, limit).await?;
            }
            PipelineCommands::Logs { repo, run_id } => {
                get_pipeline_logs(&client, &cli.server, &repo, &run_id).await?;
            }
        },
        Commands::Status { id } => {
            let url = format!("{}/api/v1/executions/{}", cli.server, id);
            let response = client.get(&url).send().await?;

            if response.status().is_success() {
                let execution: PipelineExecution = response.json().await?;
                print_execution(&execution);
            } else {
                eprintln!("Failed to get execution: {}", response.status());
                process::exit(1);
            }
        }
        Commands::Run { pulsefile, repo_url, branch } => {
            manual_run_pulsefile(&pulsefile, &repo_url, &branch).await?;
        }
        Commands::List => {
            let url = format!("{}/api/v1/executions", cli.server);
            let response = client.get(&url).send().await?;

            if response.status().is_success() {
                let executions: Vec<PipelineExecution> = response.json().await?;
                println!("Found {} execution(s):\n", executions.len());
                for exec in executions {
                    println!(
                        "  {} - {} [{}] - {}",
                        exec.id,
                        exec.pipeline_name,
                        exec.repository.full_name,
                        format_status(exec.status)
                    );
                }
            } else {
                eprintln!("Failed to list executions: {}", response.status());
                process::exit(1);
            }
        }
    }

    Ok(())
}

fn print_execution(exec: &PipelineExecution) {
    println!("Execution: {}", exec.id);
    println!("Pipeline: {} (v{})", exec.pipeline_name, exec.pipeline_version);
    println!("Repository: {}", exec.repository.full_name);
    println!("Status: {}", format_status(exec.status));
    println!("Started: {}", exec.started_at);
    if let Some(completed_at) = exec.completed_at {
        println!("Completed: {}", completed_at);
        let duration = completed_at.signed_duration_since(exec.started_at);
        println!("Duration: {:?}", duration);
    }

    println!("\nSteps:");
    for (idx, step) in exec.step_results.iter().enumerate() {
        println!("\n  {}. {} - {}", idx + 1, step.step_name, format_step_status(step.status));
        if !step.stdout.is_empty() {
            println!("     Stdout: {}", step.stdout.trim());
        }
        if !step.stderr.is_empty() {
            println!("     Stderr: {}", step.stderr.trim());
        }
        if let Some(code) = step.exit_code {
            println!("     Exit code: {}", code);
        }
        println!("     Duration: {}ms", step.duration_ms);
    }
}

fn format_status(status: pulsiora_core::PipelineStatus) -> &'static str {
    match status {
        pulsiora_core::PipelineStatus::Pending => "PENDING",
        pulsiora_core::PipelineStatus::Running => "RUNNING",
        pulsiora_core::PipelineStatus::Success => "SUCCESS",
        pulsiora_core::PipelineStatus::Failed => "FAILED",
        pulsiora_core::PipelineStatus::Cancelled => "CANCELLED",
        pulsiora_core::PipelineStatus::Skipped => "SKIPPED",
    }
}

fn format_step_status(step_status: pulsiora_core::StepStatus) -> &'static str {
    match step_status {
        pulsiora_core::StepStatus::Pending => "PENDING",
        pulsiora_core::StepStatus::Running => "RUNNING",
        pulsiora_core::StepStatus::Success => "SUCCESS",
        pulsiora_core::StepStatus::Failed => "FAILED",
        pulsiora_core::StepStatus::Skipped => "SKIPPED",
    }
}

fn generate_pulsefile_template() -> anyhow::Result<()> {
    let template = r#"# Pulsefile - Pulsiora CI/CD Pipeline Definition

pipeline {
  name: "build-and-deploy";
  version: "1.0";
  
  triggers {
    git {
      on_push: true;
      on_pull_request: true;
      on_merge: true;
      on_tag: true;
      on_release: true;
      on_branch_create: true;
      on_branch_delete: true;
      branches: ["*"];
    }
  }
  
  steps {
    step "install" {
      run: """
        npm install
        pip install -r requirements.txt
      """;
    }
    
    step "lint" {
      run: """
        npm run lint
        pylint src/
      """;
      allow_failure: true;
    }
    
    step "test" {
      run: """
        npm test
        pytest tests/
      """;
    }
    
    step "build" {
      run: """
        npm run build
        docker build -t app:latest .
      """;
    }
    
    step "deploy" {
      run: """
        ./deploy.sh
      """;
    }
  }
}
"#;

    let path = "Pulsefile";
    if Path::new(path).exists() {
        eprintln!("Error: Pulsefile already exists at {}", path);
        eprintln!("Use a different filename or remove the existing file.");
        process::exit(1);
    }

    fs::write(path, template)?;
    println!("✓ Created Pulsefile template at {}", path);
    Ok(())
}

async fn register_repo(
    client: &Client,
    server: &str,
    repo_url: &str,
    pulsefile_path: &str,
    repo_type: &str,
) -> anyhow::Result<()> {
    // Read Pulsefile
    let pulsefile_content = fs::read_to_string(pulsefile_path)
        .map_err(|e| anyhow::anyhow!("Failed to read Pulsefile at {}: {}", pulsefile_path, e))?;

    // Parse repo URL to extract owner/repo
    let repo_identifier = normalize_repo_identifier(repo_url);

    let url = format!("{}/api/v1/repos", server);
    let payload = json!({
        "repo_url": repo_url,
        "repo_identifier": repo_identifier,
        "pulsefile": pulsefile_content,
        "repo_type": repo_type,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        println!("✓ Repository registered successfully: {}", repo_url);
        println!("  Pulsefile uploaded from: {}", pulsefile_path);
    } else {
        let error_text = response.text().await.unwrap_or_default();
        eprintln!("Failed to register repository: {}", error_text);
        process::exit(1);
    }

    Ok(())
}

async fn unregister_repo(
    client: &Client,
    server: &str,
    repo_url: &str,
) -> anyhow::Result<()> {
    let repo_identifier = normalize_repo_identifier(repo_url);
    let url = format!("{}/api/v1/repos/{}", server, repo_identifier);

    let response = client.delete(&url).send().await?;

    if response.status().is_success() {
        println!("✓ Repository unregistered successfully: {}", repo_url);
    } else if response.status() == reqwest::StatusCode::NOT_FOUND {
        eprintln!("Repository not found: {}", repo_url);
        process::exit(1);
    } else {
        let error_text = response.text().await.unwrap_or_default();
        eprintln!("Failed to unregister repository: {}", error_text);
        process::exit(1);
    }

    Ok(())
}

async fn get_pipeline_status(
    client: &Client,
    server: &str,
    repo: &str,
    limit: usize,
) -> anyhow::Result<()> {
    let repo_identifier = normalize_repo_identifier(repo);
    let url = format!("{}/api/v1/pipelines/{}/status?limit={}", server, repo_identifier, limit);

    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let executions: Vec<PipelineExecution> = response.json().await?;
        println!("Recent pipeline runs for {}:\n", repo);
        
        if executions.is_empty() {
            println!("  No pipeline runs found.");
        } else {
            for exec in executions {
                println!(
                    "  {} - {} [{}] - {} - {}",
                    exec.id,
                    exec.pipeline_name,
                    exec.git_event.branch.as_ref().unwrap_or(&"N/A".to_string()),
                    format_status(exec.status),
                    exec.started_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
    } else if response.status() == reqwest::StatusCode::NOT_FOUND {
        eprintln!("Repository not found: {}", repo);
        process::exit(1);
    } else {
        let error_text = response.text().await.unwrap_or_default();
        eprintln!("Failed to get pipeline status: {}", error_text);
        process::exit(1);
    }

    Ok(())
}

async fn get_pipeline_logs(
    client: &Client,
    server: &str,
    repo: &str,
    run_id: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/api/v1/executions/{}", server, run_id);

    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let execution: PipelineExecution = response.json().await?;
        
        // Verify the execution belongs to the specified repo
        let repo_identifier = normalize_repo_identifier(repo);
        let exec_repo = normalize_repo_identifier(&execution.repository.full_name);
        
        if exec_repo != repo_identifier {
            eprintln!("Error: Run {} does not belong to repository {}", run_id, repo);
            process::exit(1);
        }
        
        print_execution(&execution);
    } else if response.status() == reqwest::StatusCode::NOT_FOUND {
        eprintln!("Pipeline run not found: {}", run_id);
        process::exit(1);
    } else {
        let error_text = response.text().await.unwrap_or_default();
        eprintln!("Failed to get pipeline logs: {}", error_text);
        process::exit(1);
    }

    Ok(())
}

fn normalize_repo_identifier(repo: &str) -> String {
    // Normalize repo URL or identifier to owner/repo format
    if repo.starts_with("http://") || repo.starts_with("https://") {
        // Extract owner/repo from URL
        let parts: Vec<&str> = repo
            .trim_end_matches(".git")
            .split('/')
            .collect();
        
        if parts.len() >= 2 {
            let owner = parts[parts.len() - 2];
            let repo_name = parts[parts.len() - 1];
            return format!("{}/{}", owner, repo_name);
        }
    }
    
    // Already in owner/repo format or just repo name
    repo.to_string()
}

async fn manual_run_pulsefile(pulsefile_path: &str, repo_url: &str, branch: &str) -> anyhow::Result<()> {
    // Read Pulsefile
    let pulsefile_content = fs::read_to_string(pulsefile_path)
        .map_err(|e| anyhow::anyhow!("Failed to read Pulsefile at {}: {}", pulsefile_path, e))?;
    
    // Parse Pulsefile
    let pipeline = parse_pulsefile(&pulsefile_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse Pulsefile: {}", e))?;
    
    println!("✅ Pulsefile parsed successfully!");
    println!("📋 Pipeline: {} v{}", pipeline.name, pipeline.version);
    println!("📁 Repository: {}", repo_url);
    println!("🌿 Branch: {}", branch);
    println!("🔢 Steps: {}", pipeline.steps.len());
    
    // Create a mock GitEvent for manual execution
    let git_event = pulsiora_core::GitEvent {
        event_type: pulsiora_core::GitEventType::Push,
        repository: pulsiora_core::Repository {
            owner: "local".to_string(),
            name: "repo".to_string(),
            full_name: repo_url.to_string(),
            clone_url: repo_url.to_string(),
            default_branch: branch.to_string(),
        },
        branch: Some(branch.to_string()),
        tag: None,
        pull_request: None,
        commit_sha: Some("manual-execution".to_string()),
        sender: "manual".to_string(),
    };
    
    println!("\n🚀 Starting manual pipeline execution...\n");
    
    // Execute the pipeline using the runner
    let executor = PipelineExecutor::new();
    let execution = executor.execute(&pipeline, &git_event).await
        .map_err(|e| anyhow::anyhow!("Pipeline execution failed: {}", e))?;
    
    println!("\n✅ Pipeline execution completed!");
    println!("📊 Status: {:?}", execution.status);
    println!("⏱️  Duration: {:?}", execution.completed_at.unwrap() - execution.started_at);
    
    if execution.status == pulsiora_core::PipelineStatus::Success {
        println!("🎉 Pipeline executed successfully!");
    } else {
        println!("❌ Pipeline failed!");
        process::exit(1);
    }
    
    Ok(())
}

