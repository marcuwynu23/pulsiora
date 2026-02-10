use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use std::collections::HashMap;
use pulsiora_core::{GitEvent, GitEventType, Repository, PipelineExecution};
use pulsiora_runner::PipelineExecutor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

mod github;
mod storage;

use github::*;
use storage::*;

#[derive(Clone)]
struct AppState {
    executor: PipelineExecutor,
    storage: Arc<RwLock<InMemoryStorage>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state = AppState {
        executor: PipelineExecutor::new(),
        storage: Arc::new(RwLock::new(InMemoryStorage::new())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/webhook/github", post(handle_github_webhook))
        .route("/api/v1/executions/:id", get(get_execution))
        .route("/api/v1/executions", get(list_executions))
        .route("/api/v1/repos", post(register_repo))
        .route("/api/v1/repos/:repo", delete(unregister_repo))
        .route("/api/v1/pipelines/:repo/status", get(get_pipeline_status))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("Server listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

#[derive(Deserialize)]
struct GitHubWebhookPayload {
    #[serde(rename = "ref")]
    ref_field: Option<String>,
    repository: Option<GitHubRepository>,
    pull_request: Option<serde_json::Value>,
    action: Option<String>,
    created: Option<bool>,
    deleted: Option<bool>,
    sender: Option<GitHubUser>,
    head_commit: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct GitHubRepository {
    name: String,
    #[serde(rename = "full_name")]
    full_name: String,
    owner: GitHubUser,
    #[serde(rename = "clone_url")]
    clone_url: String,
    #[serde(rename = "default_branch")]
    default_branch: String,
}

#[derive(Deserialize)]
struct GitHubUser {
    login: String,
}

async fn handle_github_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<GitHubWebhookPayload>,
) -> Result<StatusCode, StatusCode> {
    info!("Received GitHub webhook");

    // Determine event type from X-GitHub-Event header
    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    let repository = match &payload.repository {
        Some(repo) => Repository {
            owner: repo.owner.login.clone(),
            name: repo.name.clone(),
            full_name: repo.full_name.clone(),
            clone_url: repo.clone_url.clone(),
            default_branch: repo.default_branch.clone(),
        },
        None => {
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let git_event = match event_type {
        "push" => create_push_event(repository, &payload),
        "pull_request" => create_pull_request_event(repository, &payload),
        "create" => create_create_event(repository, &payload),
        "delete" => create_delete_event(repository, &payload),
        _ => {
            info!(event_type, "Unhandled event type, skipping");
            return Ok(StatusCode::OK);
        }
    };

    // Try to get Pulsefile from registered repos first, otherwise fetch from GitHub
    let repo_identifier = git_event.repository.full_name.clone();
    let pulsefile_content = {
        let storage = state.storage.read().await;
        if let Some(pulsefile) = storage.get_repo_pulsefile(&repo_identifier) {
            info!("Using stored Pulsefile for {}", repo_identifier);
            drop(storage);
            pulsefile
        } else {
            drop(storage);
            // Fall back to fetching from GitHub
            match fetch_pulsefile(&git_event.repository).await {
                Ok(content) => content,
                Err(e) => {
                    info!(error = %e, "Failed to fetch Pulsefile");
                    return Ok(StatusCode::OK); // Not an error, just no pipeline to run
                }
            }
        }
    };

    // Execute pipeline
    let execution = match state
        .executor
        .execute_from_pulsefile(&pulsefile_content, &git_event)
        .await
    {
        Ok(exec) => exec,
        Err(e) => {
            info!(error = %e, "Pipeline execution failed");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Store execution
    {
        let mut storage = state.storage.write().await;
        storage.store_execution(execution.clone());
    }

    info!(
        execution_id = %execution.id,
        status = ?execution.status,
        "Pipeline execution completed"
    );

    Ok(StatusCode::OK)
}

async fn get_execution(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PipelineExecution>, StatusCode> {
    let storage = state.storage.read().await;
    let execution = storage
        .get_execution(&id)
        .ok_or(StatusCode::NOT_FOUND)?
        .clone();
    Ok(Json(execution))
}

async fn list_executions(
    State(state): State<AppState>,
) -> Json<Vec<PipelineExecution>> {
    let storage = state.storage.read().await;
    Json(storage.list_executions())
}

fn create_push_event(repo: Repository, payload: &GitHubWebhookPayload) -> GitEvent {
    let branch = payload
        .ref_field
        .as_ref()
        .and_then(|r| r.strip_prefix("refs/heads/").map(String::from));

    GitEvent {
        event_type: GitEventType::Push,
        repository: repo,
        branch,
        tag: None,
        pull_request: None,
        commit_sha: payload
            .head_commit
            .as_ref()
            .and_then(|h| h.get("id"))
            .and_then(|v| v.as_str())
            .map(String::from),
        sender: payload
            .sender
            .as_ref()
            .map(|s| s.login.clone())
            .unwrap_or_default(),
    }
}

fn create_pull_request_event(repo: Repository, payload: &GitHubWebhookPayload) -> GitEvent {
    let pr = payload.pull_request.as_ref().and_then(|pr| {
        let number = pr.get("number")?.as_u64()?;
        let title = pr.get("title")?.as_str()?.to_string();
        let base = pr.get("base")?;
        let head = pr.get("head")?;
        let base_branch = base.get("ref")?.as_str()?.to_string();
        let head_branch = head.get("ref")?.as_str()?.to_string();
        let state = pr.get("state")?.as_str()?.to_string();

        Some(pulsiora_core::PullRequest {
            number,
            title,
            base_branch,
            head_branch,
            state,
        })
    });

    GitEvent {
        event_type: GitEventType::PullRequest,
        repository: repo,
        branch: None,
        tag: None,
        pull_request: pr,
        commit_sha: None,
        sender: payload
            .sender
            .as_ref()
            .map(|s| s.login.clone())
            .unwrap_or_default(),
    }
}

fn create_create_event(repo: Repository, payload: &GitHubWebhookPayload) -> GitEvent {
    let ref_str = payload.ref_field.as_deref().unwrap_or("");
    let is_tag = ref_str.starts_with("refs/tags/");
    let branch = if !is_tag {
        ref_str.strip_prefix("refs/heads/").map(String::from)
    } else {
        None
    };
    let tag = if is_tag {
        ref_str.strip_prefix("refs/tags/").map(String::from)
    } else {
        None
    };

    let event_type = if is_tag {
        GitEventType::Tag
    } else {
        GitEventType::BranchCreate
    };

    GitEvent {
        event_type,
        repository: repo,
        branch,
        tag,
        pull_request: None,
        commit_sha: None,
        sender: payload
            .sender
            .as_ref()
            .map(|s| s.login.clone())
            .unwrap_or_default(),
    }
}

fn create_delete_event(repo: Repository, payload: &GitHubWebhookPayload) -> GitEvent {
    let branch = payload
        .ref_field
        .as_ref()
        .and_then(|r| r.strip_prefix("refs/heads/").map(String::from));

    GitEvent {
        event_type: GitEventType::BranchDelete,
        repository: repo,
        branch,
        tag: None,
        pull_request: None,
        commit_sha: None,
        sender: payload
            .sender
            .as_ref()
            .map(|s| s.login.clone())
            .unwrap_or_default(),
    }
}

#[derive(Deserialize)]
struct RegisterRepoRequest {
    repo_url: String,
    repo_identifier: String,
    pulsefile: String,
    repo_type: Option<String>, // "github", "local", or other SCM type
}

#[derive(Serialize)]
struct RegisterRepoResponse {
    message: String,
    repo_identifier: String,
}

async fn register_repo(
    State(state): State<AppState>,
    Json(req): Json<RegisterRepoRequest>,
) -> Result<Json<RegisterRepoResponse>, StatusCode> {
    // Validate Pulsefile by parsing it
    if pulsiora_parser::parse_pulsefile(&req.pulsefile).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let repo_type = match req.repo_type.as_deref() {
        Some("local") => storage::RepoType::Local,
        Some(other) => storage::RepoType::Other(other.to_string()),
        None => storage::RepoType::GitHub, // Default to GitHub
    };

    let repo = storage::RegisteredRepo {
        repo_url: req.repo_url.clone(),
        repo_identifier: req.repo_identifier.clone(),
        pulsefile: req.pulsefile,
        repo_type,
    };

    {
        let mut storage = state.storage.write().await;
        storage.register_repo(repo);
    }

    info!("Registered repository: {}", req.repo_identifier);

    Ok(Json(RegisterRepoResponse {
        message: "Repository registered successfully".to_string(),
        repo_identifier: req.repo_identifier,
    }))
}

async fn unregister_repo(
    State(state): State<AppState>,
    Path(repo): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let mut storage = state.storage.write().await;
    
    if storage.unregister_repo(&repo) {
        info!("Unregistered repository: {}", repo);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}


async fn get_pipeline_status(
    State(state): State<AppState>,
    Path(repo): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<PipelineExecution>>, StatusCode> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);

    let storage = state.storage.read().await;
    let executions = storage.get_executions_by_repo(&repo, limit);

    if executions.is_empty() && !storage.is_repo_registered(&repo) {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(executions))
}

