use pulsiora_core::{
    Pipeline, Step, StepResult, StepStatus, PipelineExecution, PipelineStatus,
    GitEvent,
};
use pulsiora_parser::parse_pulsefile;
use std::path::Path;
use std::process::Command;
use chrono::Utc;
use uuid::Uuid;
use tracing::{info, warn, error};

/// Executes a pipeline from a Pulsefile
#[derive(Clone)]
pub struct PipelineExecutor {
    work_dir: Option<std::path::PathBuf>,
}

impl PipelineExecutor {
    pub fn new() -> Self {
        Self { work_dir: None }
    }

    pub fn with_work_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.work_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Execute a pipeline from a Pulsefile string
    pub async fn execute_from_pulsefile(
        &self,
        pulsefile_content: &str,
        git_event: &GitEvent,
    ) -> Result<PipelineExecution, pulsiora_core::PulsioraError> {
        let pipeline = parse_pulsefile(pulsefile_content)?;
        self.execute(&pipeline, git_event).await
    }

    /// Execute a parsed pipeline
    pub async fn execute(
        &self,
        pipeline: &Pipeline,
        git_event: &GitEvent,
    ) -> Result<PipelineExecution, pulsiora_core::PulsioraError> {
        let execution_id = Uuid::new_v4();
        let started_at = Utc::now();

        info!(
            execution_id = %execution_id,
            pipeline_name = %pipeline.name,
            "Starting pipeline execution"
        );

        // Check if pipeline should be triggered
        if !pipeline.triggers.git.matches(git_event) {
            return Ok(PipelineExecution {
                id: execution_id,
                pipeline_name: pipeline.name.clone(),
                pipeline_version: pipeline.version.clone(),
                repository: git_event.repository.clone(),
                git_event: git_event.clone(),
                status: PipelineStatus::Skipped,
                step_results: vec![],
                started_at,
                completed_at: Some(Utc::now()),
            });
        }

        let mut step_results = Vec::new();
        let mut pipeline_status = PipelineStatus::Running;

        // Execute each step in order
        for step in &pipeline.steps {
            info!(
                execution_id = %execution_id,
                step_name = %step.name,
                "Executing step"
            );

            let step_result = self.execute_step(step).await;

            if step_result.status == StepStatus::Failed && !step.allow_failure {
                pipeline_status = PipelineStatus::Failed;
                step_results.push(step_result);
                warn!(
                    execution_id = %execution_id,
                    step_name = %step.name,
                    "Step failed and allow_failure is false, stopping pipeline"
                );
                break;
            } else {
                step_results.push(step_result);
            }
        }

        // Determine final status
        if pipeline_status == PipelineStatus::Running {
            let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
            pipeline_status = if has_failures {
                PipelineStatus::Failed
            } else {
                PipelineStatus::Success
            };
        }

        let completed_at = Utc::now();

        info!(
            execution_id = %execution_id,
            pipeline_name = %pipeline.name,
            status = ?pipeline_status,
            "Pipeline execution completed"
        );

        Ok(PipelineExecution {
            id: execution_id,
            pipeline_name: pipeline.name.clone(),
            pipeline_version: pipeline.version.clone(),
            repository: git_event.repository.clone(),
            git_event: git_event.clone(),
            status: pipeline_status,
            step_results,
            started_at,
            completed_at: Some(completed_at),
        })
    }

    async fn execute_step(&self, step: &Step) -> StepResult {
        let started_at = Utc::now();
        let start_instant = std::time::Instant::now();

        info!(step_name = %step.name, "Executing step command");

        // Execute the step's run command
        // For simplicity, we'll execute commands in a shell
        // In production, you'd want to handle different shells and environments
        
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg(&step.run)
                .current_dir(self.work_dir.as_ref().map(|p| p.as_path()).unwrap_or_else(|| std::path::Path::new(".")))
                .output()
        } else {
            Command::new("sh")
                .arg("-c")
                .arg(&step.run)
                .current_dir(self.work_dir.as_ref().map(|p| p.as_path()).unwrap_or_else(|| std::path::Path::new(".")))
                .output()
        };

        let duration_ms = start_instant.elapsed().as_millis() as u64;
        let completed_at = Utc::now();

        match output {
            Ok(output) => {
                let status = if output.status.success() {
                    StepStatus::Success
                } else {
                    StepStatus::Failed
                };

                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code();

                info!(
                    step_name = %step.name,
                    status = ?status,
                    exit_code = ?exit_code,
                    "Step execution completed"
                );

                StepResult {
                    step_name: step.name.clone(),
                    status,
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms,
                    started_at,
                    completed_at: Some(completed_at),
                }
            }
            Err(e) => {
                error!(
                    step_name = %step.name,
                    error = %e,
                    "Step execution failed"
                );

                StepResult {
                    step_name: step.name.clone(),
                    status: StepStatus::Failed,
                    stdout: String::new(),
                    stderr: format!("Failed to execute command: {}", e),
                    exit_code: None,
                    duration_ms,
                    started_at,
                    completed_at: Some(completed_at),
                }
            }
        }
    }
}

impl Default for PipelineExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsiora_core::{GitEventType, Repository};

    fn create_test_repo() -> Repository {
        Repository {
            owner: "test".to_string(),
            name: "repo".to_string(),
            full_name: "test/repo".to_string(),
            clone_url: "https://github.com/test/repo.git".to_string(),
            default_branch: "main".to_string(),
        }
    }

    fn create_test_event() -> GitEvent {
        GitEvent {
            event_type: GitEventType::Push,
            repository: create_test_repo(),
            branch: Some("main".to_string()),
            tag: None,
            pull_request: None,
            commit_sha: None,
            sender: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_executor_skips_non_matching_trigger() {
        let executor = PipelineExecutor::new();
        
        let pulsefile = r#"
pipeline {
  name: "test";
  triggers {
    git {
      on_push: false;
    }
  }
  steps {
    step "test" {
      run: """echo "test"""";
    }
  }
}
"#;
        
        let execution = executor
            .execute_from_pulsefile(pulsefile, &create_test_event())
            .await
            .unwrap();
        
        assert_eq!(execution.status, PipelineStatus::Skipped);
        assert_eq!(execution.step_results.len(), 0);
    }

    #[tokio::test]
    async fn test_executor_executes_matching_pipeline() {
        let executor = PipelineExecutor::new();
        
        let pulsefile = r#"
pipeline {
  name: "test";
  triggers {
    git {
      on_push: true;
      branches: ["main"];
    }
  }
  steps {
    step "test" {
      run: """echo "hello world"""";
    }
  }
}
"#;
        
        let execution = executor
            .execute_from_pulsefile(pulsefile, &create_test_event())
            .await
            .unwrap();
        
        assert_eq!(execution.status, PipelineStatus::Success);
        assert_eq!(execution.step_results.len(), 1);
        assert_eq!(execution.step_results[0].step_name, "test");
        assert_eq!(execution.step_results[0].status, StepStatus::Success);
    }

    #[tokio::test]
    async fn test_executor_stops_on_failure() {
        let executor = PipelineExecutor::new();
        
        let pulsefile = r#"
pipeline {
  name: "test";
  triggers {
    git {
      on_push: true;
    }
  }
  steps {
    step "failing" {
      run: """exit 1""";
    }
    step "should_not_run" {
      run: """echo "should not run"""";
    }
  }
}
"#;
        
        let execution = executor
            .execute_from_pulsefile(pulsefile, &create_test_event())
            .await
            .unwrap();
        
        assert_eq!(execution.status, PipelineStatus::Failed);
        assert_eq!(execution.step_results.len(), 1);
    }

    #[tokio::test]
    async fn test_executor_continues_on_allow_failure() {
        let executor = PipelineExecutor::new();
        
        let pulsefile = r#"
pipeline {
  name: "test";
  triggers {
    git {
      on_push: true;
    }
  }
  steps {
    step "failing" {
      run: """exit 1""";
      allow_failure: true;
    }
    step "success" {
      run: """echo "success"""";
    }
  }
}
"#;
        
        let execution = executor
            .execute_from_pulsefile(pulsefile, &create_test_event())
            .await
            .unwrap();
        
        assert_eq!(execution.status, PipelineStatus::Success);
        assert_eq!(execution.step_results.len(), 2);
        assert_eq!(execution.step_results[0].status, StepStatus::Failed);
        assert_eq!(execution.step_results[1].status, StepStatus::Success);
    }

    #[tokio::test]
    async fn test_executor_multiple_steps() {
        let executor = PipelineExecutor::new();
        
        let pulsefile = r#"
pipeline {
  name: "test";
  triggers {
    git {
      on_push: true;
    }
  }
  steps {
    step "step1" {
      run: """echo "step1"""";
    }
    step "step2" {
      run: """echo "step2"""";
    }
    step "step3" {
      run: """echo "step3"""";
    }
  }
}
"#;
        
        let execution = executor
            .execute_from_pulsefile(pulsefile, &create_test_event())
            .await
            .unwrap();
        
        assert_eq!(execution.status, PipelineStatus::Success);
        assert_eq!(execution.step_results.len(), 3);
        assert_eq!(execution.step_results[0].step_name, "step1");
        assert_eq!(execution.step_results[1].step_name, "step2");
        assert_eq!(execution.step_results[2].step_name, "step3");
    }
}
