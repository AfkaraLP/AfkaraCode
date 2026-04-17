use std::{path::Path, process::Command, time::Duration};

use colored::Colorize;
use openai_client::{IntoPinBox, ToolCallArgDescriptor, ToolCallFn};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

use crate::config::load_config;
use crate::utils::{
    create_file, edit_file, list_directory_contents, make_directory, read_file_with_range,
};

pub struct EditFile;
pub struct ReadFile;
pub struct CreateFile;
pub struct ListDirectoryContents;
pub struct MakeDirectory;
pub struct BashExec;

impl ToolCallFn for EditFile {
    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }
    fn get_args(&self) -> Vec<ToolCallArgDescriptor> {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        let Some(Value::String(old)) = args.get("old") else {
            return "please provide the old snippet that should be replaced.".into_pin_box();
        };
        let Some(Value::String(new)) = args.get("new") else {
            return "please provide a new snippet to be inserted in the file".into_pin_box();
        };

        println!(
            "{} {}\n{} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "edit_file".bold().truecolor(0, 188, 212),
            "Path:".bold().green(),
            path
        );

        // Show a colored diff preview between old and new snippets
        let diff = TextDiff::from_lines(old, new);
        println!("{}", "Diff:".bold().truecolor(255, 215, 0));
        for op in diff.ops() {
            for change in diff.iter_changes(op) {
                match change.tag() {
                    ChangeTag::Delete => print!("{}", format!("-{change}").red()),
                    ChangeTag::Insert => print!("{}", format!("+{change}").green()),
                    ChangeTag::Equal => print!(" {}", change.to_string().truecolor(150, 150, 150)),
                }
            }
        }
        println!();

        match edit_file(path.clone(), old, new) {
            Ok(v) => {
                // Also highlight the new snippet using syntax inferred from the path
                let ps: SyntaxSet = SyntaxSet::load_defaults_newlines();
                let ts: ThemeSet = ThemeSet::load_defaults();
                let theme = ts
                    .themes
                    .get("base16-ocean.dark")
                    .unwrap_or_else(|| ts.themes.values().next().expect("has theme"));
                let ext = Path::new(&path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let syntax = ps
                    .find_syntax_by_extension(ext)
                    .or_else(|| ps.find_syntax_by_name("Rust"))
                    .unwrap_or_else(|| ps.find_syntax_plain_text());
                let mut h = HighlightLines::new(syntax, theme);
                println!(
                    "{}",
                    "applied changes (preview):".bold().truecolor(0, 188, 212)
                );
                for line in LinesWithEndings::from(new) {
                    let ranges = h
                        .highlight_line(line, &ps)
                        .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    print!("{escaped}");
                }
                println!();
                // Run formatter hook after successful edit
                if let Err(e) = run_formatter_hook(&path) {
                    println!("{} {}", "formatter error:".bold().red(), e);
                }
                println!("{} {}", "✔".green(), v.truecolor(102, 187, 106));
                v.into_pin_box()
            }
            Err(e) => {
                println!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}
fn run_formatter_hook(path: &str) -> Result<(), String> {
    // Load config
    let cfg = load_config();
    if !cfg.formatter_enabled() {
        return Ok(());
    }

    // Determine extension
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Find command
    let cmd_template = cfg
        .command_for_ext(&ext)
        .or_else(|| default_formatter_for_ext(&ext))
        .ok_or_else(|| format!("no formatter configured for .{ext}"))?;

    // Build command by replacing {file}
    let command_line = cmd_template.replace("{file}", path);

    // Execute via shell for convenience so templates can contain args
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", &command_line]).status()
    } else {
        Command::new("sh").args(["-lc", &command_line]).status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("formatter exited with status {s}")),
        Err(e) => Err(format!("failed to run formatter: {e}")),
    }
}

fn default_formatter_for_ext(ext: &str) -> Option<String> {
    match ext {
        // Rust
        "rs" => Some("rustfmt {file}".to_string()),
        // JavaScript/TypeScript and variants
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => Some("prettier --write {file}".to_string()),
        // JSON, CSS, Markdown supported by Prettier too
        "json" | "css" | "scss" | "md" | "markdown" | "yaml" | "yml" => {
            Some("prettier --write {file}".to_string())
        }
        // Python
        "py" => Some("ruff format {file}".to_string()),
        // Go
        "go" => Some("gofmt -w {file}".to_string()),
        // Shell
        "sh" | "bash" => Some("shfmt -w {file}".to_string()),
        // Lua
        "lua" => Some("stylua {file}".to_string()),
        // C/C++
        "c" | "h" | "cpp" | "cxx" | "hpp" => Some("clang-format -i {file}".to_string()),
        // Ruby
        "rb" => Some(
            "rubocop -x --only Layout/EndAlignment,Layout/SpaceInsideBlockBraces {file}"
                .to_string(),
        ),
        // PHP
        "php" => Some("php-cs-fixer fix {file}".to_string()),
        // Default: none
        _ => None,
    }
}

impl ToolCallFn for ReadFile {
    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }
    fn get_args(&self) -> Vec<ToolCallArgDescriptor> {
        vec![
            ToolCallArgDescriptor::string("path", "the relative path of the file to read."),
            ToolCallArgDescriptor::number(
                "offset",
                "optional byte offset to start reading from (defaults to 0)",
            ),
            ToolCallArgDescriptor::number(
                "length",
                "optional maximum number of bytes to read (reads to EOF if omitted)",
            ),
        ]
    }

    fn get_description(&self) -> &'static str {
        "read the contents of a file with optional byte offset and length. always use this to know how a file looks like before modifying it in any way shape or form."
    }

    fn get_name(&self) -> &'static str {
        "read_file"
    }

    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        let offset: Option<u64> = args.get("offset").and_then(serde_json::Value::as_u64);
        let length: Option<usize> = args
            .get("length")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| usize::try_from(n).ok());

        println!("reading file at path: {path} (offset={offset:?}, length={length:?})");
        match read_file_with_range(path.clone(), offset, length) {
            Ok(v) => {
                let bytes = v.len();
                format!(
                    "read file path: {path} (offset={offset:?}, length={length:?}, bytes_read={bytes})"
                )
                .into_pin_box()
            }
            Err(e) => {
                println!("failed reading file: {e}");
                e.into_pin_box()
            }
        }
    }
}
impl ToolCallFn for CreateFile {
    fn get_args(&self) -> Vec<ToolCallArgDescriptor> {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        let Some(Value::String(content)) = args.get("content") else {
            return "please provide content".into_pin_box();
        };
        println!(
            "{} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "create_file".bold().truecolor(0, 188, 212),
            format!("path={path}").italic().blue(),
        );
        match create_file(path.clone(), content.clone()) {
            Ok(v) => {
                // Run formatter hook after successful edit
                if let Err(e) = run_formatter_hook(&path) {
                    println!("{} {}", "formatter error:".bold().red(), e);
                }
                println!("{} {}", "✔".green(), v.truecolor(102, 187, 106));
                v.into_pin_box()
            }
            Err(e) => {
                println!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}
fn apply_filters(
    text: &str,
    for_re: Option<&regex::Regex>,
    out_re: Option<&regex::Regex>,
) -> String {
    text.lines()
        .filter(|line| for_re.is_none_or(|r| r.is_match(line)))
        .filter(|line| out_re.is_none_or(|r| !r.is_match(line)))
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

async fn run_bash_command(
    cmd: String,
    cwd: Option<String>,
    timeout_ms: u64,
    filter_for: Option<String>,
    filter_out: Option<String>,
) -> String {
    use tokio::process::Command;
    use tokio::time::{Duration, timeout};

    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&cmd);
        c
    } else {
        let mut c = Command::new("bash");
        c.arg("-lc").arg(&cmd);
        c
    };

    if let Some(ref dir) = cwd {
        println!("{} {}", "working dir:".bold().magenta(), dir);
        command.current_dir(dir);
    }

    println!("{}", "starting process...".bold().truecolor(121, 134, 203));

    let output_fut = command.output();
    let result = timeout(Duration::from_millis(timeout_ms), output_fut).await;

    // Prepare regex filters if provided and valid
    let filter_for_re = filter_for.and_then(|pat| match regex::Regex::new(&pat) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("{} {}", "invalid filter_for regex:".bold().red(), e);
            None
        }
    });
    let filter_out_re = filter_out.and_then(|pat| match regex::Regex::new(&pat) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("{} {}", "invalid filter_out regex:".bold().red(), e);
            None
        }
    });

    match result {
        Ok(Ok(output)) => {
            let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_raw = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = apply_filters(&stdout_raw, filter_for_re.as_ref(), filter_out_re.as_ref());
            let stderr = apply_filters(&stderr_raw, filter_for_re.as_ref(), filter_out_re.as_ref());
            let code = output.status.code().unwrap_or(-1);
            let status_text = if code == 0 {
                "completed".green()
            } else {
                "failed".red()
            };
            println!("{}", status_text.bold());
            serde_json::json!({
                "exit_code": code,
                "stdout": stdout,
                "stderr": stderr,
            })
            .to_string()
        }
        Ok(Err(e)) => {
            println!("{} {}", "failed to execute command:".bold().red(), e);
            "failed to execute command".to_string()
        }
        Err(_) => {
            println!(
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

impl ToolCallFn for BashExec {
    fn get_args(&self) -> Vec<ToolCallArgDescriptor> {
        vec![
            ToolCallArgDescriptor::string("cmd", "the exact shell command to execute")
                .set_required(),
            ToolCallArgDescriptor::string("cwd", "optional working directory; defaults to current")
                .set_optional(),
            ToolCallArgDescriptor::string(
                "timeout_ms",
                "optional timeout in milliseconds; defaults to 60000",
            )
            .set_optional(),
            ToolCallArgDescriptor::string(
                "filter_for",
                "optional regex: only include lines that match this (applied to stdout/stderr)",
            )
            .set_optional(),
            ToolCallArgDescriptor::string(
                "filter_out",
                "optional regex: exclude lines that match this (applied to stdout/stderr)",
            )
            .set_optional(),
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(cmd)) = args.get("cmd") else {
            return "please provide a cmd".into_pin_box();
        };
        let cwd = args
            .get("cwd")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let timeout_ms = args
            .get("timeout_ms")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60_000);

        let filter_for = args
            .get("filter_for")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let filter_out = args
            .get("filter_out")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);

        println!(
            "{} {} {} {} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "bash_exec".bold().truecolor(0, 188, 212),
            "cmd:".bold().yellow(),
            cmd,
            "cwd:".bold().yellow(),
            format!("{cwd:?}").italic().blue()
        );

        Box::pin(run_bash_command(
            cmd.clone(),
            cwd,
            timeout_ms,
            filter_for,
            filter_out,
        ))
    }
}
impl ToolCallFn for ListDirectoryContents {
    fn get_args(&self) -> Vec<ToolCallArgDescriptor> {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        println!(
            "{} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "list_dir_contents".bold().truecolor(0, 188, 212),
            format!("path={path}").italic().blue(),
        );
        match list_directory_contents(path.clone()) {
            Ok(v) => {
                println!(
                    "{} {}",
                    "✔".green(),
                    "successfully listed dir contents".truecolor(102, 187, 106)
                );
                v.into_pin_box()
            }
            Err(e) => {
                println!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}

impl ToolCallFn for MakeDirectory {
    fn get_args(&self) -> Vec<ToolCallArgDescriptor> {
        vec![ToolCallArgDescriptor::string(
            "path",
            "the absolute or relative path of the directory to create",
        )]
    }

    fn get_description(&self) -> &'static str {
        "create a directory at the specified path (including intermediate directories if necessary)"
    }

    fn get_name(&self) -> &'static str {
        "make_dir"
    }

    fn get_timeout_wait(&self) -> std::time::Duration {
        Duration::ZERO
    }

    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        let Some(Value::String(path)) = args.get("path") else {
            return "please provide a path".into_pin_box();
        };
        println!(
            "{} {} {}",
            "[tool]".bold().truecolor(255, 193, 7),
            "make_dir".bold().truecolor(0, 188, 212),
            format!("path={path}").italic().blue(),
        );
        match make_directory(path.clone()) {
            Ok(v) => {
                // Run formatter hook after successful edit
                if let Err(e) = run_formatter_hook(&path) {
                    println!("{} {}", "formatter error:".bold().red(), e);
                }
                println!("{} {}", "✔".green(), v.truecolor(102, 187, 106));
                v.into_pin_box()
            }
            Err(e) => {
                println!("{} {}", "✖".red(), e.truecolor(239, 83, 80));
                e.into_pin_box()
            }
        }
    }
}
