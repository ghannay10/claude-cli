mod chat;

use chat::chat;
use reqwest::Client;
use tokio;
use tokio::{runtime, signal};

fn main() {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_client());
}

async fn run_client() {
    let client = Client::new();
    let ctrl_c = signal::ctrl_c();

    tokio::select! {
        _ = ctrl_c => {
            println!("Ctrl+C pressed. Exiting...");
            return;
        }
        _ = continuous_chat(&client) => {}
    }
}

async fn continuous_chat(client: &Client) {
    loop {
        chat(&client).await
    }
}
