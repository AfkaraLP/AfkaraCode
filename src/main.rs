mod env;
mod render;
mod tools;
mod utils;
mod lua;
mod xdg;
mod config;

use std::io::Write;

fn workspace_extra_instructions() -> Option<String> {
    use std::fs;
    use std::time::SystemTime;

    let files = ["AGENTS.md", "CLAUDE.md"];
    let mut newest: Option<(&str, SystemTime)> = None;
    for name in files {
        if let Ok(meta) = fs::metadata(name) {
            if let Ok(mtime) = meta.modified() {
                match newest {
                    None => newest = Some((name, mtime)),
                    Some((_, ref t)) if mtime > *t => newest = Some((name, mtime)),
                    _ => {}
                }
            }
        }
    }

    if let Some((name, _)) = newest {
        if let Ok(content) = fs::read_to_string(name) {
            return Some(format!(
                "\n\nAdditional workspace instructions (from {name}):\n\n{}",
                content
            ));
        }
    }
    None
}

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

    let mut tools = ToolMap::new()
        .register_tool(EditFile)
        .register_tool(ReadFile)
        .register_tool(CreateFile)
        .register_tool(MakeDirectory)
        .register_tool(ListDirectoryContents)
        .register_tool(BashExec);

    // Load user-defined Lua tools from XDG config plugin dirs and local ./lua_tools
    let mut lua_tool_dirs = crate::xdg::plugin_dirs();
    lua_tool_dirs.push(std::path::PathBuf::from("lua_tools"));
    for t in crate::lua::load_lua_tools_from_dirs(&lua_tool_dirs) {
        tools = tools.register_tool(t);
    }

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
        let cmd = prompt.trim();
        if cmd == "exit" || cmd == ":q" || cmd == ":x" {
            break;
        }

        // Initialize with system + first user, then append subsequent user turns
        if messages.is_empty() {
            let mut system_prompt = String::from("You are a coding agent. Don't edit files unless specifically instructed to. When reading files, prefer using the optional offset and length arguments on read_file to avoid reading the entire file unless necessary.");
            if let Some(extra) = workspace_extra_instructions() {
                system_prompt.push_str(&extra);
            }
            messages = new_system_user_turn(
                &system_prompt,
                prompt.clone(),
            );
        } else {
            messages.push(ChatCompletionMessageParam::new_user(prompt.clone()));
        }

        let max_retries: u32 = 5;
        let result = {
            let mut attempt = 0u32;
            loop {
                match client.run_agent(messages.clone(), &tools).await {
                    Ok(r) => break Some(r),
                    Err(e) => {
                        attempt += 1;
                        if attempt >= max_retries {
                            eprintln!("Failed after {max_retries} attempts: {e:?}");
                            break None;
                        }
                        let delay_secs = 2u64.pow(attempt);
                        eprintln!(
                            "Request failed (attempt {attempt}/{max_retries}): {e:?}\nRetrying in {delay_secs}s..."
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    }
                }
            }
        };

        let Some((resp, history)) = result else {
            eprintln!("Skipping this turn due to repeated failures.");
            continue;
        };

        // Update history so the agent retains context
        messages = history;

        // Render agent response with Markdown code-fence syntax highlighting
        let agent_header = "### 🤖 Agent".bold().truecolor(137, 207, 240);
        let border = "────────────────────────────────────────────────".truecolor(80, 80, 80);
        let rendered = render_markdown_to_terminal(&resp.content.unwrap_or_default());
        println!("\n{agent_header}\n{border}\n{rendered}\n{border}\n");
    }
}
