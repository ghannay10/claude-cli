use colored::*;
use futures::stream::StreamExt;
use reqwest::{Client, Response};
use serde_json::{json, Value};
use std::{env, io, str};

fn get_api_key() -> String {
    match env::var("CLAUDE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error getting CLAUDE_API_KEY environment variable");
            std::process::exit(1);
        }
    }
}

fn handle_user_input(args: Vec<String>) -> String {
    let mut input = String::new();

    if args.len() < 2 {
        println!("\n\nYour query: ('exit' to quit)");

        let mut first_line = String::new();
        io::stdin()
            .read_line(&mut first_line)
            .expect("Failed to read line");
        if first_line.starts_with("'''") {
            loop {
                let mut line = String::new();
                io::stdin()
                    .read_line(&mut line)
                    .expect("Failed to read line");

                // Break the loop if the user enters the closing marker '''
                if line.ends_with("'''\n") {
                    break;
                }

                // Append the line to the overall input
                input.push_str(&line);
            }
        } else if first_line.trim().to_lowercase() == "exit" {
            std::process::exit(0);
        } else {
            input = first_line;
        }
    } else {
        input = args[1..].join(" ");
    }

    input
}

async fn get_response(client: &Client, conversation: &Vec<Value>) -> Response {
    let params = serde_json::to_string(&json!({
       "model": "claude-2.1",
       "messages": conversation,
       "max_tokens": 1024,
       "stream": true
    }))
    .unwrap();

    let api_key = get_api_key();

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "messages-2023-12-15")
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .body(params)
        .send()
        .await
        .unwrap();

    response
}

fn print_response(text: String, full_text: &String) {
    let count = full_text.matches("```").count();
    if count % 2 == 0 {
        if text.matches("```").count() == 1 {
            print!("{}", text.truecolor(42, 136, 192));
        } else {
            print!("{}", text.truecolor(0, 154, 116));
        }
    } else {
        print!("{}", text.truecolor(42, 136, 192));
    }
}

async fn handle_conversation(response: Response) -> String {
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut full_text = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.unwrap(); // Handle chunk error if needed
        buffer.extend_from_slice(&chunk);

        let buffer_str = match str::from_utf8(&buffer) {
            Ok(v) => v,
            Err(_) => continue, // Not valid UTF-8 yet, wait for more data
        };

        let lines: Vec<&str> = buffer_str
            .split("\n")
            .filter(|line| !line.is_empty())
            .collect();

        for line in lines {
            let parts: Vec<&str> = line.splitn(2, "data: ").collect();
            if parts.len() == 2 {
                if let Ok(json) = serde_json::from_str::<Value>(parts[1]) {
                    if json["type"].as_str() == Some("content_block_delta")
                        && json["delta"]["type"].as_str() == Some("text_delta")
                    {
                        let delta = &json["delta"];
                        if let Some(text) = delta["text"].as_str() {
                            full_text.push_str(text);

                            print_response(text.to_string(), &full_text);
                        }
                    }
                }
            }
        }
        buffer.clear(); // Clear the buffer after processing
    }
    full_text
}

pub async fn chat(client: &Client) {
    let mut conversation_history = vec![];
    let mut args: Vec<String> = env::args().collect();

    loop {
        let input = handle_user_input(args);
        args = vec![]; // clear args

        conversation_history.push(json!({"role": "user", "content": input}));

        let response = get_response(&client, &conversation_history).await;
        let full_text = handle_conversation(response).await;

        conversation_history.push(json!({"role": "assistant", "content": full_text}));
    }
}

#[tokio::test]
async fn test_get_api_key() {
    env::set_var("CLAUDE_API_KEY", "mock_api_key");
    assert_eq!(get_api_key(), "mock_api_key");
}

#[test]
fn test_handle_user_input() {
    let input = vec!["test".to_string()];
    assert_eq!(handle_user_input(input), "test");

    let input = vec![];
    assert_eq!(handle_user_input(input), "");

    let input = vec!["test".to_string(), "input".to_string()];
    assert_eq!(handle_user_input(input), "test input");
}

#[tokio::test]
pub async fn test_get_response() {
    // Request a new server from the pool
    let mut server = mockito::Server::new();

    // Use one of these addresses to configure your client
    let _m = server
        .mock("POST", "v1/messages")
        .match_header("anthropic-version", "2023-06-01")
        .match_header("anthropic-beta", "messages-2023-12-15")
        .match_header("Content-Type", "application/json")
        .match_header("x-api-key", "mock_api_key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status": "success"}"#)
        .create();

    let client = Client::new();
    env::set_var("CLAUDE_API_KEY", "mock_api_key");
    let conversation = vec![json!({"role": "user", "content": "test"})];
    let response = get_response(&client, &conversation).await;
    assert_eq!(response.status(), 200);
}
