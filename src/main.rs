mod config;
mod env;
mod lua;
mod render;
mod tools;
mod utils;
mod xdg;

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
use crossterm::{
    event::{self, Event, KeyCode},
    terminal,
};
use openai_client::{
    ChatCompletionMessageParam, OpenAIAuth, OpenAIClient, ToolMap, new_system_user_turn,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

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
            let mut system_prompt = String::from(
                "You are a careful, fast coding agent.\n- Never modify files unless the user explicitly asks for changes.\n- To inspect files, always use read_file first and prefer offset/length to avoid large reads.\n- Use list_dir_contents to explore directories before reading. Keep answers concise.",
            );
            if let Some(extra) = workspace_extra_instructions() {
                system_prompt.push_str(&extra);
            }
            messages = new_system_user_turn(&system_prompt, prompt.clone());
        } else {
            messages.push(ChatCompletionMessageParam::new_user(prompt.clone()));
        }

        let max_retries: u32 = 5;
        let mut was_cancelled = false;
        let result = {
            let mut attempt = 0u32;
            loop {
                // Spawn an ESC listener thread for this attempt
                use std::time::Duration as StdDuration;
                let cancelled = Arc::new(AtomicBool::new(false));
                let running = Arc::new(AtomicBool::new(true));
                let cancelled_thread = Arc::clone(&cancelled);
                let running_thread = Arc::clone(&running);

                // TODO: we got rid of raw mode for now as it messed up printing, escape handling has bad ux right now, fix
                let esc_handle = std::thread::spawn(move || {
                    loop {
                        if !running_thread.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Ok(true) = event::poll(StdDuration::from_millis(100)) {
                            if let Ok(Event::Key(key)) = event::read() {
                                if key.code == KeyCode::Esc {
                                    cancelled_thread.store(true, Ordering::SeqCst);
                                    break;
                                }
                            }
                        }
                    }
                });

                // Build the agent future and a cancellation future
                let mut fut = Box::pin(client.run_agent(messages.clone(), &tools));
                let cancel_fut = async {
                    while !cancelled.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                };

                let mut retry_delay: Option<u64> = None;
                let outcome: Option<(
                    openai_client::ChatCompletionResponseMessage,
                    Vec<ChatCompletionMessageParam>,
                )> = tokio::select! {
                    res = &mut fut => {
                        running.store(false, Ordering::SeqCst);
                        match res {
                            Ok(r) => Some(r),
                            Err(e) => {
                                attempt += 1;
                                if attempt >= max_retries {
                                    eprintln!("Failed after {max_retries} attempts: {e:?}");
                                    None
                                } else {
                                    let delay_secs = 2u64.pow(attempt);
                                    eprintln!(
                                        "Request failed (attempt {attempt}/{max_retries}): {e:?}\nRetrying in {delay_secs}s..."
                                    );
                                    retry_delay = Some(delay_secs);
                                    None
                                }
                            }
                        }
                    },
                    _ = cancel_fut => {
                        // User pressed ESC; stop the ESC thread and mark cancelled
                        running.store(false, Ordering::SeqCst);
                        was_cancelled = true;
                        None
                    }
                };

                // Ensure ESC thread is stopped and joined before proceeding
                let _ = esc_handle.join();

                // If we have a result or were cancelled, break out of the retry loop
                if was_cancelled || outcome.is_some() {
                    break outcome;
                }

                // If we are retrying, wait the delay and continue the attempt loop
                if let Some(secs) = retry_delay {
                    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                    continue;
                }

                // No result and no retry scheduled -> break with None
                break outcome;
            }
        };

        if result.is_none() {
            if was_cancelled {
                eprintln!("Inference cancelled by user (ESC).\n");
            } else {
                eprintln!("Skipping this turn due to repeated failures.");
            }
            continue;
        }

        let (resp, history) = result.unwrap();

        // Update history so the agent retains context
        messages = history;

        // Render agent response with Markdown code-fence syntax highlighting
        let agent_header = "### Afkara Agent".bold().truecolor(137, 207, 240);
        let border = "────────────────────────────────────────────────".truecolor(80, 80, 80);
        let rendered = render_markdown_to_terminal(&resp.content.unwrap_or_default());
        println!("\n{agent_header}\n{border}\n{rendered}\n{border}\n");
    }
}
