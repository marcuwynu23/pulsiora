use crate::grammar::{PulsefileParser, Rule};
use pulsiora_core::{GitTriggers, Pipeline, Step, Triggers, PulsioraError, Result};
use pest::Parser;

/// Parse a Pulsefile string into a Pipeline structure
pub fn parse_pulsefile(input: &str) -> Result<Pipeline> {
    let mut pairs = PulsefileParser::parse(Rule::file, input)
        .map_err(|e| PulsioraError::ParseError(format!("Parse error: {}", e)))?;

    let pipeline_pair = pairs.next().ok_or_else(|| {
        PulsioraError::ParseError("No pipeline found in file".to_string())
    })?;

    parse_pipeline(pipeline_pair)
}

fn parse_pipeline(pair: pest::iterators::Pair<Rule>) -> Result<Pipeline> {
    let mut name = String::new();
    let mut version = String::new();
    let mut triggers = None;
    let mut steps = Vec::new();

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::pipeline_metadata => {
                let (parsed_name, parsed_version) = parse_pipeline_metadata(inner_pair)?;
                if !parsed_name.is_empty() {
                    name = parsed_name;
                }
                if !parsed_version.is_empty() {
                    version = parsed_version;
                }
            }
            Rule::triggers => {
                triggers = Some(parse_triggers(inner_pair)?);
            }
            Rule::steps => {
                steps = parse_steps(inner_pair)?;
            }
            _ => {}
        }
    }

    Ok(Pipeline {
        name: if name.is_empty() { "default".to_string() } else { name },
        version: if version.is_empty() { "1.0".to_string() } else { version },
        triggers: triggers.unwrap_or_else(|| Triggers {
            git: GitTriggers::default(),
        }),
        steps,
    })
}

fn parse_pipeline_metadata(pair: pest::iterators::Pair<Rule>) -> Result<(String, String)> {
    let mut name = String::new();
    let mut version = String::new();

    let text = pair.as_str();
    
    // Extract name field
    if let Some(start) = text.find("name:") {
        if let Some(end) = text[start..].find(";") {
            let value_str = &text[start + 5..start + end];
            if let Some(quote_start) = value_str.find('"') {
                if let Some(quote_end) = value_str[quote_start + 1..].find('"') {
                    name = unquote_string(&value_str[quote_start..quote_start + quote_end + 2]);
                }
            }
        }
    }
    
    // Extract version field
    if let Some(start) = text.find("version:") {
        if let Some(end) = text[start..].find(";") {
            let value_str = &text[start + 8..start + end];
            if let Some(quote_start) = value_str.find('"') {
                if let Some(quote_end) = value_str[quote_start + 1..].find('"') {
                    version = unquote_string(&value_str[quote_start..quote_start + quote_end + 2]);
                }
            }
        }
    }

    Ok((name, version))
}

fn parse_triggers(pair: pest::iterators::Pair<Rule>) -> Result<Triggers> {
    let mut git_triggers = GitTriggers::default();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::git {
            git_triggers = parse_git_triggers(inner_pair)?;
        }
    }

    Ok(Triggers { git: git_triggers })
}

fn parse_git_triggers(pair: pest::iterators::Pair<Rule>) -> Result<GitTriggers> {
    let mut triggers = GitTriggers::default();
    let text = pair.as_str();

    // Parse each trigger field by searching the text
    triggers.on_push = parse_boolean_field(text, "on_push");
    triggers.on_pull_request = parse_boolean_field(text, "on_pull_request");
    triggers.on_merge = parse_boolean_field(text, "on_merge");
    triggers.on_tag = parse_boolean_field(text, "on_tag");
    triggers.on_release = parse_boolean_field(text, "on_release");
    triggers.on_branch_create = parse_boolean_field(text, "on_branch_create");
    triggers.on_branch_delete = parse_boolean_field(text, "on_branch_delete");

    // Parse branches
    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::branch_list {
            triggers.branches = parse_branch_list(inner_pair)?;
        }
    }

    Ok(triggers)
}

fn parse_boolean_field(text: &str, field_name: &str) -> bool {
    if let Some(start) = text.find(&format!("{}:", field_name)) {
        if let Some(end) = text[start..].find(";") {
            let value_str = text[start + field_name.len() + 1..start + end].trim();
            return value_str == "true";
        }
    }
    false
}

fn parse_branch_list(pair: pest::iterators::Pair<Rule>) -> Result<Vec<String>> {
    let mut branches = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::string_literal {
            branches.push(unquote_string(inner_pair.as_str()));
        }
    }

    Ok(branches)
}

fn parse_steps(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Step>> {
    let mut steps = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::step {
            steps.push(parse_step(inner_pair)?);
        }
    }

    Ok(steps)
}

fn parse_step(pair: pest::iterators::Pair<Rule>) -> Result<Step> {
    let mut name = String::new();
    let mut run = String::new();
    let mut allow_failure = false;

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::string_literal => {
                // First string_literal is the step name
                if name.is_empty() {
                    name = unquote_string(inner_pair.as_str());
                }
            }
            Rule::multiline_string => {
                run = unquote_multiline_string(inner_pair.as_str());
            }
            Rule::boolean => {
                allow_failure = inner_pair.as_str() == "true";
            }
            _ => {}
        }
    }

    Ok(Step {
        name,
        run: run.trim().to_string(),
        allow_failure,
    })
}

fn unquote_string(s: &str) -> String {
    s.trim_matches('"').to_string()
}

fn unquote_multiline_string(s: &str) -> String {
    s.trim()
        .strip_prefix("\"\"\"")
        .unwrap_or(s)
        .strip_suffix("\"\"\"")
        .unwrap_or(s)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pipeline() {
        let input = r#"
pipeline {
  name: "test-pipeline";
  version: "1.0";
  triggers {
    git {
      on_push: true;
      branches: ["main"];
    }
  }
  steps {
    step "test" {
      run: """
        echo "test"
      """;
    }
  }
}
"#;
        let pipeline = parse_pulsefile(input).unwrap();
        assert_eq!(pipeline.name, "test-pipeline");
        assert_eq!(pipeline.version, "1.0");
        assert!(pipeline.triggers.git.on_push);
        assert_eq!(pipeline.triggers.git.branches, vec!["main"]);
        assert_eq!(pipeline.steps.len(), 1);
        assert_eq!(pipeline.steps[0].name, "test");
    }

    #[test]
    fn test_parse_complex_pipeline() {
        let input = r#"
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
        let pipeline = parse_pulsefile(input).unwrap();
        assert_eq!(pipeline.name, "build-and-deploy");
        assert_eq!(pipeline.version, "1.0");
        assert!(pipeline.triggers.git.on_push);
        assert!(pipeline.triggers.git.on_pull_request);
        assert!(pipeline.triggers.git.on_tag);
        assert_eq!(pipeline.triggers.git.branches, vec!["*"]);
        assert_eq!(pipeline.steps.len(), 5);
        assert_eq!(pipeline.steps[0].name, "install");
        assert_eq!(pipeline.steps[1].name, "lint");
        assert!(pipeline.steps[1].allow_failure);
        assert!(!pipeline.steps[0].allow_failure);
    }

    #[test]
    fn test_parse_minimal_pipeline() {
        let input = r#"
pipeline {
  triggers {
    git {
    }
  }
  steps {
  }
}
"#;
        let pipeline = parse_pulsefile(input).unwrap();
        assert_eq!(pipeline.name, "default");
        assert_eq!(pipeline.version, "1.0");
        assert_eq!(pipeline.steps.len(), 0);
    }

    #[test]
    fn test_parse_with_multiline_string() {
        let input = r#"
pipeline {
  name: "test";
  triggers {
    git {
      on_push: true;
    }
  }
  steps {
    step "multiline" {
      run: """
        echo "line 1"
        echo "line 2"
        echo "line 3"
      """;
    }
  }
}
"#;
        let pipeline = parse_pulsefile(input).unwrap();
        assert_eq!(pipeline.steps.len(), 1);
        let run_content = pipeline.steps[0].run.clone();
        assert!(run_content.contains("line 1"));
        assert!(run_content.contains("line 2"));
        assert!(run_content.contains("line 3"));
    }

    #[test]
    fn test_parse_invalid_syntax() {
        let input = "invalid syntax here";
        assert!(parse_pulsefile(input).is_err());
    }

    #[test]
    fn test_parse_empty_file() {
        let input = "";
        assert!(parse_pulsefile(input).is_err());
    }

    #[test]
    fn test_parse_multiple_steps_with_allow_failure() {
        let input = r#"
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
      allow_failure: true;
    }
    step "step3" {
      run: """echo "step3"""";
      allow_failure: false;
    }
  }
}
"#;
        let pipeline = parse_pulsefile(input).unwrap();
        assert_eq!(pipeline.steps.len(), 3);
        assert!(!pipeline.steps[0].allow_failure);
        assert!(pipeline.steps[1].allow_failure);
        assert!(!pipeline.steps[2].allow_failure);
    }
}
