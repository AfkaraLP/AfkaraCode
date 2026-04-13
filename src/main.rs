use std::{
    io::{Read, Write},
    time::Duration,
};

use openai_client::{IntoPinBox, OpenAIClient, ToolCallArgDescriptor, ToolCallFn, ToolMap};
use serde_json::Value;

#[tokio::main]
async fn main() {
    let tools = ToolMap::new()
        .register_tool(EditFile)
        .register_tool(ReadFile)
        .register_tool(ListDirectoryContents);
    let client = OpenAIClient::new(
        "http://localhost:1234/v1",
        "nvidia/nemotron-3-nano-4b",
        None,
    );
    loop {
        let mut prompt = String::new();
        print!("AfkaraCode> ");
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
        println!("{resp}");
    }
}

pub struct EditFile;
pub struct ReadFile;
pub struct ListDirectoryContents;

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

        eprintln!("editing file {path} with:\n{old}\nto:\n{new}");
        match edit_file(path.clone(), old, new) {
            Ok(v) => {
                eprintln!("successfully edited file: {v}");
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("failed editing file: {e}");
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
        match read_file(path.to_string()) {
            Ok(v) => {
                eprintln!("successfully read file: {v}");
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("failed reading file: {e}");
                e.into_pin_box()
            }
        }
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
        eprintln!("listing dir contents at path: {path}");
        match list_directory_contents(path.to_string()) {
            Ok(v) => {
                eprintln!("successfully listed dir contents: {v}");
                v.into_pin_box()
            }
            Err(e) => {
                eprintln!("failed listing dir contents: {e}");
                e.into_pin_box()
            }
        }
    }
}

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

pub fn read_file(path: String) -> Result<String, &'static str> {
    std::fs::read_to_string(path).map_err(|_| "failed reading file.")
}

pub fn list_directory_contents(path: String) -> Result<String, &'static str> {
    Ok(std::fs::read_dir(path)
        .map_err(|_| "failed to read directory contents.")?
        .into_iter()
        .fold(String::new(), |mut acc, entry| {
            match entry {
                Ok(v) => acc.push_str(
                    v.file_name()
                        .to_str()
                        .unwrap_or("unknown error reading this entry."),
                ),
                Err(_) => acc.push_str("unknown error reading this entry."),
            };
            acc
        }))
}
