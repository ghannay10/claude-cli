use colored::*;
use dialoguer::{theme::ColorfulTheme, Input};
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::str;
use tokio;
use tokio::runtime;
use tokio::signal;

use futures::stream::StreamExt; // Import StreamExt for stream handling

fn main() {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main());
}

async fn async_main() {
    let client = Client::new();
    let ctrl_c = signal::ctrl_c();

    tokio::select! {
        _ = ctrl_c => {
            println!("Ctrl+C pressed. Exiting...");
            return;
        }
        _ = run_client(&client) => {}
    }
}

async fn run_client(client: &Client) {
    loop {
        chat(&client).await
    }
}

async fn chat(client: &Client) {
    let mut conversation_history = vec![];
    let mut args: Vec<String> = env::args().collect();
    loop {
        let input: String;
        if args.len() < 2 {
            input = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Your query:")
                .interact_text()
                .unwrap();
        } else {
            input = args[1..].join(" ");
            args = vec![]; // clear args
        }

        if input == "exit" {
            std::process::exit(0);
        }

        conversation_history.push(json!({"role": "user", "content": input}));

        let params = serde_json::to_string(&json!({
           "model": "claude-2.1",
           "messages": conversation_history.clone(),
           "max_tokens": 256,
           "stream": true
        }))
        .unwrap();

        let api_key: String = match env::var("CLAUDE_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Error getting CLAUDE_API_KEY environment variable");
                std::process::exit(1);
            }
        };

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

                                let count = full_text.matches("```").count();
                                if count % 2 == 0 {
                                    if text.matches("```").count() == 1 {
                                        print!("{}", text.truecolor(42, 136, 192));
                                    } else {
                                        print!("{}", text);
                                    }
                                } else {
                                    print!("{}", text.truecolor(42, 136, 192));
                                }
                            }
                        }
                    }
                }
            }
            buffer.clear(); // Clear the buffer after processing
        }

        conversation_history.push(json!({"role": "assistant", "content": full_text}));
    }
}
