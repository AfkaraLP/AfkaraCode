mod env;
mod render;
mod tools;
mod utils;

use std::io::Write;

use colored::Colorize;
use openai_client::{
    ChatCompletionMessageParam, OpenAIAuth, OpenAIClient, ToolMap, new_system_user_turn,
};

use crate::env::ENV_VARS;
use crate::render::render_markdown_to_terminal;
use crate::tools::{
    BashExec, CreateFile, EditFile, ListDirectoryContents, MakeDirectory, ReadFile,
};

#[tokio::main]
async fn main() {
    let model_name = "gpt-5";
    let endpoint = &ENV_VARS.v1_endpoint;
    let api_key: Option<OpenAIAuth> = ENV_VARS.api_key.clone();

    let tools = ToolMap::new()
        .register_tool(EditFile)
        .register_tool(ReadFile)
        .register_tool(CreateFile)
        .register_tool(MakeDirectory)
        .register_tool(ListDirectoryContents)
        .register_tool(BashExec);

    let client = OpenAIClient::new(endpoint, model_name, api_key);

    // Fancy, colorful prompt label
    let prompt_label = "AfkaraCode> ".bold().truecolor(255, 111, 97);

    // Persist conversation history across turns
    let mut messages: Vec<ChatCompletionMessageParam> = Vec::new();

    loop {
        let mut prompt = String::new();
        print!("{prompt_label}");
        _ = std::io::stdout().flush();
        _ = std::io::stdin().read_line(&mut prompt);
        if prompt.contains("exit") {
            break;
        }

        // Initialize with system + first user, then append subsequent user turns
        if messages.is_empty() {
            messages = new_system_user_turn(
                "You are a coding agent. Don't edit files unless specifically instructed to. When reading files, prefer using the optional offset and length arguments on read_file to avoid reading the entire file unless necessary.",
                prompt.clone(),
            );
        } else {
            messages.push(ChatCompletionMessageParam::new_user(prompt.clone()));
        }

        let (resp, history) = client.run_agent(messages.clone(), &tools).await.unwrap();

        // Update history so the agent retains context
        messages = history;

        // Render agent response with Markdown code-fence syntax highlighting
        let agent_header = "### 🤖 Agent".bold().truecolor(137, 207, 240);
        let border = "────────────────────────────────────────────────".truecolor(80, 80, 80);
        let rendered = render_markdown_to_terminal(&resp.content.unwrap_or_default());
        println!("\n{agent_header}\n{border}\n{rendered}\n{border}\n");
    }
}
