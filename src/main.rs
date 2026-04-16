mod env;
mod render;
mod tools;
mod utils;

use std::io::Write;

use colored::Colorize;
use openai_client::{OpenAIAuth, OpenAIClient, ToolMap};

use crate::env::ENV_VARS;
use crate::render::render_markdown_to_terminal;
use crate::tools::{BashExec, CreateFile, EditFile, ListDirectoryContents, MakeDirectory, ReadFile};

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

    loop {
        let mut prompt = String::new();
        print!("{}", prompt_label);
        _ = std::io::stdout().flush();
        _ = std::io::stdin().read_line(&mut prompt);
        if prompt.contains("exit") {
            break;
        }
        let resp = client
            .run_agent(
                "You are a coding agent. Don't edit files unless specifically instructed to. When reading files, prefer using the optional offset and length arguments on read_file to avoid reading the entire file unless necessary.",
                prompt,
                &tools,
            )
            .await
            .unwrap();

        // Render agent response with Markdown code-fence syntax highlighting
        let agent_header = "### 🤖 Agent".bold().truecolor(137, 207, 240);
        let border = "────────────────────────────────────────────────".truecolor(80, 80, 80);
        let rendered = render_markdown_to_terminal(&resp);
        println!("\n{}\n{}\n{}\n{}\n",
            agent_header,
            border,
            rendered,
            border);
    }
}
