use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Represents a complete pipeline definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pipeline {
    pub name: String,
    pub version: String,
    pub triggers: Triggers,
    pub steps: Vec<Step>,
}

/// Trigger configuration for a pipeline
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Triggers {
    pub git: GitTriggers,
}

/// Git event triggers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitTriggers {
    pub on_push: bool,
    pub on_pull_request: bool,
    pub on_merge: bool,
    pub on_tag: bool,
    pub on_release: bool,
    pub on_branch_create: bool,
    pub on_branch_delete: bool,
    pub branches: Vec<String>, // Supports patterns like "*", "main", "feature/*"
}

/// A pipeline step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Step {
    pub name: String,
    pub run: String,
    pub allow_failure: bool,
}

/// Git event types that can trigger pipelines
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GitEventType {
    Push,
    PullRequest,
    Merge,
    Tag,
    Release,
    BranchCreate,
    BranchDelete,
}

impl From<&str> for GitEventType {
    fn from(s: &str) -> Self {
        match s {
            "push" => GitEventType::Push,
            "pull_request" => GitEventType::PullRequest,
            "merge" => GitEventType::Merge,
            "tag" => GitEventType::Tag,
            "release" => GitEventType::Release,
            "branch_create" => GitEventType::BranchCreate,
            "branch_delete" => GitEventType::BranchDelete,
            _ => GitEventType::Push, // Default
        }
    }
}

/// Represents a Git event from GitHub
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitEvent {
    pub event_type: GitEventType,
    pub repository: Repository,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub pull_request: Option<PullRequest>,
    pub commit_sha: Option<String>,
    pub sender: String,
}

/// Repository information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
}

/// Pull request information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub base_branch: String,
    pub head_branch: String,
    pub state: String,
}

/// Execution status of a step
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Result of step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_name: String,
    pub status: StepStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Pipeline execution status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PipelineStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
    Skipped,
}

/// Complete pipeline execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecution {
    pub id: Uuid,
    pub pipeline_name: String,
    pub pipeline_version: String,
    pub repository: Repository,
    pub git_event: GitEvent,
    pub status: PipelineStatus,
    pub step_results: Vec<StepResult>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Default for GitTriggers {
    fn default() -> Self {
        Self {
            on_push: false,
            on_pull_request: false,
            on_merge: false,
            on_tag: false,
            on_release: false,
            on_branch_create: false,
            on_branch_delete: false,
            branches: vec!["*".to_string()],
        }
    }
}

impl Step {
    pub fn new(name: String, run: String) -> Self {
        Self {
            name,
            run,
            allow_failure: false,
        }
    }

    pub fn with_allow_failure(mut self, allow: bool) -> Self {
        self.allow_failure = allow;
        self
    }
}

impl GitTriggers {
    /// Check if a git event should trigger this pipeline
    pub fn matches(&self, event: &GitEvent) -> bool {
        // Check event type
        let event_matches = match event.event_type {
            GitEventType::Push => self.on_push,
            GitEventType::PullRequest => self.on_pull_request,
            GitEventType::Merge => self.on_merge,
            GitEventType::Tag => self.on_tag,
            GitEventType::Release => self.on_release,
            GitEventType::BranchCreate => self.on_branch_create,
            GitEventType::BranchDelete => self.on_branch_delete,
        };

        if !event_matches {
            return false;
        }

        // Check branch filter
        if let Some(ref branch) = event.branch {
            self.matches_branch(branch)
        } else if event.tag.is_some() {
            // For tag events, we check if on_tag is enabled
            self.on_tag
        } else {
            true
        }
    }

    /// Check if a branch matches the configured branch patterns
    pub fn matches_branch(&self, branch: &str) -> bool {
        if self.branches.is_empty() {
            return false;
        }

        self.branches.iter().any(|pattern| {
            if pattern == "*" {
                return true;
            }
            if pattern == branch {
                return true;
            }
            // Simple glob pattern matching (e.g., "feature/*")
            if pattern.ends_with("/*") {
                let prefix = &pattern[..pattern.len() - 2];
                return branch.starts_with(prefix);
            }
            false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_repo() -> Repository {
        Repository {
            owner: "test".to_string(),
            name: "repo".to_string(),
            full_name: "test/repo".to_string(),
            clone_url: "https://github.com/test/repo.git".to_string(),
            default_branch: "main".to_string(),
        }
    }

    #[test]
    fn test_git_triggers_default() {
        let triggers = GitTriggers::default();
        assert!(!triggers.on_push);
        assert_eq!(triggers.branches, vec!["*".to_string()]);
    }

    #[test]
    fn test_git_triggers_matches_branch_wildcard() {
        let triggers = GitTriggers {
            branches: vec!["*".to_string()],
            ..Default::default()
        };
        assert!(triggers.matches_branch("main"));
        assert!(triggers.matches_branch("feature/abc"));
    }

    #[test]
    fn test_git_triggers_matches_branch_specific() {
        let triggers = GitTriggers {
            branches: vec!["main".to_string()],
            ..Default::default()
        };
        assert!(triggers.matches_branch("main"));
        assert!(!triggers.matches_branch("develop"));
    }

    #[test]
    fn test_git_triggers_matches_branch_pattern() {
        let triggers = GitTriggers {
            branches: vec!["feature/*".to_string()],
            ..Default::default()
        };
        assert!(triggers.matches_branch("feature/abc"));
        assert!(triggers.matches_branch("feature/xyz"));
        assert!(!triggers.matches_branch("main"));
    }

    #[test]
    fn test_git_triggers_matches_event() {
        let triggers = GitTriggers {
            on_push: true,
            branches: vec!["main".to_string()],
            ..Default::default()
        };

        let event = GitEvent {
            event_type: GitEventType::Push,
            repository: create_test_repo(),
            branch: Some("main".to_string()),
            tag: None,
            pull_request: None,
            commit_sha: None,
            sender: "user".to_string(),
        };

        assert!(triggers.matches(&event));
    }

    #[test]
    fn test_git_triggers_no_match_wrong_event() {
        let triggers = GitTriggers {
            on_push: true,
            on_pull_request: false,
            ..Default::default()
        };

        let event = GitEvent {
            event_type: GitEventType::PullRequest,
            repository: create_test_repo(),
            branch: None,
            tag: None,
            pull_request: None,
            commit_sha: None,
            sender: "user".to_string(),
        };

        assert!(!triggers.matches(&event));
    }

    #[test]
    fn test_step_new() {
        let step = Step::new("test".to_string(), "echo hello".to_string());
        assert_eq!(step.name, "test");
        assert_eq!(step.run, "echo hello");
        assert!(!step.allow_failure);
    }

    #[test]
    fn test_step_with_allow_failure() {
        let step = Step::new("test".to_string(), "echo hello".to_string())
            .with_allow_failure(true);
        assert!(step.allow_failure);
    }
}
