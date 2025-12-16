use pulsiora_core::{Repository, PulsioraError, Result};
use reqwest::Client;

pub async fn fetch_pulsefile(repository: &Repository) -> Result<String> {
    let client = Client::new();
    
    // Construct GitHub raw content URL
    // For now, we'll fetch from the default branch
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/Pulsefile",
        repository.full_name,
        repository.default_branch
    );

    info!("Fetching Pulsefile from: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PulsioraError::NetworkError(format!("Failed to fetch Pulsefile: {}", e)))?;

    if !response.status().is_success() {
        return Err(PulsioraError::PipelineNotFound(format!(
            "Pulsefile not found in repository {}",
            repository.full_name
        )));
    }

    let content = response
        .text()
        .await
        .map_err(|e| PulsioraError::NetworkError(format!("Failed to read Pulsefile: {}", e)))?;

    Ok(content)
}

use tracing::info;

