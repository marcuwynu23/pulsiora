// Process execution utilities
// Future extension point for more sophisticated process management

pub struct ProcessConfig {
    pub timeout: Option<std::time::Duration>,
    pub env: Vec<(String, String)>,
    pub working_directory: Option<std::path::PathBuf>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            timeout: None,
            env: Vec::new(),
            working_directory: None,
        }
    }
}

