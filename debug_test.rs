use pulsiora_parser::parse_pulsefile;

fn main() {
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
    
    match parse_pulsefile(input) {
        Ok(pipeline) => println!("Success: {:?}", pipeline),
        Err(e) => println!("Error: {}", e),
    }
}