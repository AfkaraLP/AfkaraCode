use std::{
    io::{Read, Write},
    sync::LazyLock,
    time::Duration,
};

use colored::{Color, Colorize};
use openai_client::{
    IntoPinBox, OpenAIAuth, OpenAIClient, ToolCallArgDescriptor, ToolCallFn, ToolMap,
};
use serde_json::Value;

#[must_use]
#[inline]
pub fn openai_auth_from_string(string: String) -> Option<OpenAIAuth> {
    if string.contains('|') {
        return string.split_once('|').map(|(k, v)| OpenAIAuth::ApiKey {
            key: k.to_string(),
            value: v.to_string(),
        });
    }
    Some(OpenAIAuth::BearerToken(string))
}

pub struct EnvVars {
    pub api_key: Option<OpenAIAuth>,
    pub v1_endpoint: String,
}

pub static ENV_VARS: LazyLock<EnvVars> = LazyLock::new(|| {
    let api_key: Option<OpenAIAuth> = dotenvy::var("API_KEY")
        .ok()
        .and_then(openai_auth_from_string);
    let v1_endpoint = dotenvy::var("V1_ENDPOINT").expect("Please provied a v1 endpoint in .env");
    EnvVars {
        api_key,
        v1_endpoint,
    }
});

#[tokio::main]
async fn main() {
    let model_name = "gpt-5";
    let endpoint = &ENV_VARS.v1_endpoint;
    let api_key: Option<OpenAIAuth> = ENV_VARS.api_key.clone();

    let tools = ToolMap::new()
        .register_tool(EditFile)
        .register_tool(ReadFile)
        .register_tool(CreateFile)
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
                "You are a coding agent, don't edit files unless specifically instructed to",
                prompt,
                &tools,
            )
            .await
            .unwrap();

        // Colorize the agent response in Markdown style blocks with rainbow-ish accents
        let agent_header = "### 🤖 Agent".bold().truecolor(137, 207, 240);
        let border = "────────────────────────────────────────────────".truecolor(80, 80, 80);
        let resp_colored = resp
            .replace("```", "```") // ensure code fences remain intact
            .truecolor(220, 220, 220);
        println!("\n{}\n{}\n{}\n{}\n",
            agent_header,
            border,
            resp_colored,
            border);
    }
}

pub struct EditFile;
pub struct ReadFile;
pub struct CreateFile;
pub struct ListDirectoryContents;
pub struct BashExec;

impl ToolCallFn for EditFile {
    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }
    fn get_args(&self) -> Vec<openai_client::ToolCallArgDescriptor> {
        vec![
            ToolCallArgDescriptor::string("path", "the relative path of the file to read."),
            ToolCallArgDescriptor::string("old", "the part of the file that should be replaced"),
            ToolCallArgDescriptor::string("new", "the new snippet that should be used in the file"),
        ]
    }

    fn get_description(&self) -> &'static str {
        "edit a file by providing what snippet of the file you want to change and what to replate it with. this uses str replace."
    }

    fn get_name(&self) -> &'static str {
        "edit_file"
    }

    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        let Some(Value::String(old)) = args.get("old") else {
            return "please provide the old snippet that should be replaced.".into_pin_box();
        };
        let Some(Value::String(new)) = args.get("new") else {
            return "please provide a new snippet to be inserted in the file".into_pin_box();
        };

        eprintln!(
            "{} {}\n{} {}\n{} {}\n{} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "edit_file".bold().truecolor(0, 188, 212),
            "Path:".bold().green(), path,
            "Old:".bold().yellow(), old,
            "New:".bold().cyan(), new
        );
        match edit_file(path.clone(), old, new) {
            Ok(v) => {
                eprintln!("{} {}", "✔".green(), v.truecolor(102, 187, 106));
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}
impl ToolCallFn for ReadFile {
    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }
    fn get_args(&self) -> Vec<openai_client::ToolCallArgDescriptor> {
        vec![ToolCallArgDescriptor::string(
            "path",
            "the relative path of the file to read.",
        )]
    }

    fn get_description(&self) -> &'static str {
        "read the contents of a file. always use this to know how a file looks like before modifying it in any way shape or form."
    }

    fn get_name(&self) -> &'static str {
        "read_file"
    }

    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        eprintln!("reading file at path: {path}");
        match read_file(path.clone()) {
            Ok(v) => {
                // Only color the "file output:" label, not the file contents
                println!("{}", "file output:".bold().truecolor(0, 188, 212));
                println!("{}", v);
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("failed reading file: {e}");
                e.into_pin_box()
            }
        }
    }
}
impl ToolCallFn for CreateFile {
    fn get_args(&self) -> Vec<openai_client::ToolCallArgDescriptor> {
        vec![
            ToolCallArgDescriptor::string("path", "the relative path of the file to create."),
            ToolCallArgDescriptor::string("content", "the content to write to the file."),
        ]
    }

    fn get_description(&self) -> &'static str {
        "create a new file with the given content at the given path."
    }

    fn get_name(&self) -> &'static str {
        "create_file"
    }

    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }
    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        let Some(Value::String(content)) = args.get("content") else {
            return "please provide content".into_pin_box();
        };
        eprintln!(
            "{} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "create_file".bold().truecolor(0, 188, 212),
            format!("path={}", path).italic().blue(),
        );
        match create_file(path.clone(), content.clone()) {
            Ok(v) => {
                eprintln!("{} {}", "✔".green(), v.truecolor(102, 187, 106));
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}
impl ToolCallFn for BashExec {
    fn get_args(&self) -> Vec<openai_client::ToolCallArgDescriptor> {
        vec![
            ToolCallArgDescriptor::string("cmd", "the exact shell command to execute"),
            ToolCallArgDescriptor::string("cwd", "optional working directory; defaults to current"),
            ToolCallArgDescriptor::string(
                "timeout_ms",
                "optional timeout in milliseconds; defaults to 60000",
            ),
        ]
    }

    fn get_description(&self) -> &'static str {
        "execute an arbitrary bash command in a shell and return stdout, stderr and exit code"
    }

    fn get_name(&self) -> &'static str {
        "bash_exec"
    }

    fn get_timeout_wait(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }

    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn Future<Output = String> + Send + 'invocation>> {
        use tokio::process::Command;
        use tokio::time::{Duration, timeout};

        let Some(Value::String(cmd)) = args.get("cmd") else {
            return "please provide a cmd".into_pin_box();
        };
        let cwd = args
            .get("cwd")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let timeout_ms = args
            .get("timeout_ms")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60_000);

        eprintln!(
            "{} {} {} {} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "bash_exec".bold().truecolor(0, 188, 212),
            "cmd:".bold().yellow(), cmd,
            "cwd:".bold().yellow(), format!("{:?}", cwd).italic().blue()
        );
        eprintln!(
            "{} {}",
            "timeout_ms:".bold().yellow(), timeout_ms
        );

        async fn run(cmd: String, cwd: Option<String>, timeout_ms: u64) -> String {
            let mut command = if cfg!(target_os = "windows") {
                let mut c = Command::new("cmd");
                c.arg("/C").arg(cmd);
                c
            } else {
                let mut c = Command::new("bash");
                c.arg("-lc").arg(cmd);
                c
            };

            if let Some(dir) = cwd.clone() {
                eprintln!("{} {}", "working dir:".bold().magenta(), dir);
                command.current_dir(dir);
            }

            eprintln!("{}", "starting process...".bold().truecolor(121, 134, 203));

            let output_fut = command.output();
            let result = timeout(Duration::from_millis(timeout_ms), output_fut).await;

            match result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let code = output.status.code().unwrap_or(-1);
                    let exit_color = if code == 0 { Color::Green } else { Color::Red };
                    eprintln!(
                        "{} {} {} {}",
                        "completed:".bold().truecolor(76, 175, 80),
                        format!("{} {}", "exit_code".bold().color(exit_color), code),
                        format!("{} {}", "stdout_len:".bold().cyan(), stdout.len()).cyan(),
                        format!("{} {}", "stderr_len:".bold().cyan(), stderr.len()).cyan(),
                    );
                    // Pretty-print a preview of stdout/stderr with colors
                    if !stdout.is_empty() {
                        eprintln!("\n{}\n{}\n{}\n",
                            "stdout:".bold().green(),
                            stdout.clone(),
                            "─".repeat(20).truecolor(60,60,60),
                        );
                    }
                    if !stderr.is_empty() {
                        eprintln!("\n{}\n{}\n{}\n",
                            "stderr:".bold().red(),
                            stderr.clone(),
                            "─".repeat(20).truecolor(60,60,60),
                        );
                    }
                    serde_json::json!({
                        "exit_code": code,
                        "stdout": stdout,
                        "stderr": stderr,
                    })
                    .to_string()
                }
                Ok(Err(e)) => {
                    eprintln!("{} {}", "failed to execute command:".bold().red(), e);
                    "failed to execute command".to_string()
                }
                Err(_) => {
                    eprintln!(
                        "{} {} {}",
                        "timed out after".bold().red(),
                        timeout_ms,
                        "ms".bold().red()
                    );
                    serde_json::json!({
                        "error": "timeout",
                        "timeout_ms": timeout_ms,
                    })
                    .to_string()
                }
            }
        }

        Box::pin(run(cmd.clone(), cwd, timeout_ms))
    }
}
impl ToolCallFn for ListDirectoryContents {
    fn get_args(&self) -> Vec<openai_client::ToolCallArgDescriptor> {
        vec![ToolCallArgDescriptor::string(
            "path",
            "the absolute or relative path of the directory to list",
        )]
    }

    fn get_description(&self) -> &'static str {
        "list the contents of a directory on an absolute or relative path. use this to explore codebases"
    }

    fn get_name(&self) -> &'static str {
        "list_dir_contents"
    }

    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }
    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        eprintln!(
            "{} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "list_dir_contents".bold().truecolor(0, 188, 212),
            format!("path={}", path).italic().blue(),
        );
        match list_directory_contents(path.clone()) {
            Ok(v) => {
                eprintln!("{} {}", "✔".green(), "successfully listed dir contents".truecolor(102, 187, 106));
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}

/// # Errors
///
/// - Old string not found in file.
/// - File cannot be opened.
pub fn edit_file(
    path: impl AsRef<str>,
    old: &str,
    new: &str,
) -> Result<&'static str, &'static str> {
    let mut file = std::fs::File::open(path.as_ref()).map_err(|_| "couldn't open file.")?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)
        .map_err(|_| "failed to read file.")?;

    if !file_content.contains(old) {
        return Err("file did not contain old string.");
    }

    let file_content = file_content.replace(old, new);

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path.as_ref())
        .map_err(|_| "failed to reopen file for writing.")?;

    file.write_all(file_content.as_bytes())
        .map_err(|_| "failed to write to file.")?;

    Ok("successfully wrote to file")
}

/// # Errors
///
/// Errors if file is read protected or does not exist.
pub fn read_file(path: String) -> Result<String, &'static str> {
    std::fs::read_to_string(path).map_err(|_| "failed reading file.")
}

/// # Errors
///
/// Can error if the directory we're trying to create the file in is write protected.
pub fn create_file(path: String, content: String) -> Result<String, &'static str> {
    std::fs::write(path, content).map_err(|_| "failed to create file")?;
    Ok("successfully created file".to_string())
}

/// # Errors
///
/// Can error if the directory cannot be read from or does not exist.
pub fn list_directory_contents(path: String) -> Result<String, &'static str> {
    use std::fmt::Write;
    Ok(std::fs::read_dir(path)
        .map_err(|_| "failed to read directory contents.")?
        .fold(String::new(), |mut acc, entry| {
            match entry {
                Ok(v) => {
                    _ = writeln!(
                        &mut acc,
                        "{}",
                        v.file_name()
                            .to_str()
                            .unwrap_or("unknown error reading this entry.")
                    );
                }

                Err(_) => acc.push_str("unknown error reading this entry.\n"),
            }
            acc
        }))
}
