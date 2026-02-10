use pulsiora_core::PipelineExecution;
use std::collections::HashMap;
use uuid::Uuid;

/// Repository type
#[derive(Debug, Clone, PartialEq)]
pub enum RepoType {
    GitHub,
    Local,
    Other(String), // Other SCM systems
}

/// Repository registration information
#[derive(Debug, Clone)]
pub struct RegisteredRepo {
    pub repo_url: String,
    pub repo_identifier: String, // owner/repo format
    pub pulsefile: String,
    pub repo_type: RepoType,
}

/// In-memory storage for pipeline executions and registered repos
/// In production, this would be replaced with a database
pub struct InMemoryStorage {
    executions: HashMap<Uuid, PipelineExecution>,
    registered_repos: HashMap<String, RegisteredRepo>, // key: repo_identifier
    executions_by_repo: HashMap<String, Vec<Uuid>>, // repo_identifier -> execution IDs
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            executions: HashMap::new(),
            registered_repos: HashMap::new(),
            executions_by_repo: HashMap::new(),
        }
    }

    pub fn store_execution(&mut self, execution: PipelineExecution) {
        let repo_id = execution.repository.full_name.clone();
        self.executions.insert(execution.id, execution.clone());
        
        // Track executions by repo
        self.executions_by_repo
            .entry(repo_id)
            .or_insert_with(Vec::new)
            .push(execution.id);
    }

    pub fn get_execution(&self, id: &str) -> Option<&PipelineExecution> {
        let uuid = Uuid::parse_str(id).ok()?;
        self.executions.get(&uuid)
    }

    pub fn list_executions(&self) -> Vec<PipelineExecution> {
        self.executions.values().cloned().collect()
    }

    pub fn get_executions_by_repo(&self, repo_identifier: &str, limit: usize) -> Vec<PipelineExecution> {
        let execution_ids = self.executions_by_repo
            .get(repo_identifier)
            .cloned()
            .unwrap_or_default();
        
        let mut executions: Vec<_> = execution_ids
            .iter()
            .filter_map(|id| self.executions.get(id).cloned())
            .collect();
        
        // Sort by started_at descending (most recent first)
        executions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        
        executions.into_iter().take(limit).collect()
    }

    pub fn register_repo(&mut self, repo: RegisteredRepo) {
        self.registered_repos.insert(repo.repo_identifier.clone(), repo);
    }

    pub fn unregister_repo(&mut self, repo_identifier: &str) -> bool {
        self.registered_repos.remove(repo_identifier).is_some()
    }

    pub fn get_repo_pulsefile(&self, repo_identifier: &str) -> Option<String> {
        self.registered_repos
            .get(repo_identifier)
            .map(|r| r.pulsefile.clone())
    }

    pub fn is_repo_registered(&self, repo_identifier: &str) -> bool {
        self.registered_repos.contains_key(repo_identifier)
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsiora_core::{GitEvent, GitEventType, Repository, PipelineStatus};
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_execution(id: Uuid) -> PipelineExecution {
        let repo = Repository {
            owner: "test".to_string(),
            name: "repo".to_string(),
            full_name: "test/repo".to_string(),
            clone_url: "https://github.com/test/repo.git".to_string(),
            default_branch: "main".to_string(),
        };

        let event = GitEvent {
            event_type: GitEventType::Push,
            repository: repo.clone(),
            branch: Some("main".to_string()),
            tag: None,
            pull_request: None,
            commit_sha: None,
            sender: "test".to_string(),
        };

        PipelineExecution {
            id,
            pipeline_name: "test".to_string(),
            pipeline_version: "1.0".to_string(),
            repository: repo,
            git_event: event,
            status: PipelineStatus::Success,
            step_results: vec![],
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
        }
    }

    #[test]
    fn test_storage_store_and_retrieve() {
        let mut storage = InMemoryStorage::new();
        let id = Uuid::new_v4();
        let execution = create_test_execution(id);

        storage.store_execution(execution.clone());
        let retrieved = storage.get_execution(&id.to_string());

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, id);
    }

    #[test]
    fn test_storage_not_found() {
        let storage = InMemoryStorage::new();
        let id = Uuid::new_v4();

        assert!(storage.get_execution(&id.to_string()).is_none());
    }

    #[test]
    fn test_storage_list_executions() {
        let mut storage = InMemoryStorage::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        storage.store_execution(create_test_execution(id1));
        storage.store_execution(create_test_execution(id2));

        let executions = storage.list_executions();
        assert_eq!(executions.len(), 2);
    }
}
