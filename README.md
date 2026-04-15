# afkaracode

> [!NOTE]
> This readme was created with the agent. Claude code -- rather crap code -- I'm coming for your money!!!

*Hash of the first commit done solely by the agent:* `0d293b9`

## Overview

afkaracode is a Rust-based AI coding assistant that provides an interactive command-line interface for working with files using function calling. It leverages OpenAI's function calling system to execute custom tools that allow you to:

- Read file contents
- Create new files  
- Edit existing files by replacing specific snippets
- List directory contents

The project demonstrates how an AI can act as a coding agent that can directly manipulate files on disk.

## Features

- Interactive loop: type commands and get responses from the AI agent
- File manipulation tools:
  - `edit_file`: Replace old snippet with new snippet in a file
  - `read_file`: Read file contents (with optional offset and length)
  - `create_file`: Create a new file with given content
  - `list_directory_contents`: List files and directories in a path

## How It Works

1. Run the program: `cargo run`
2. The AI agent runs locally at `http://localhost:1234/v1`
3. Type commands like:
   ```
   AfkaraCode> read_file README.md
   AfkaraCode> read_file README.md offset=0 length=200
   ```
4. The AI will execute the tool call and return results

## Requirements

- Rust 1.56+
- OpenAI function calling endpoint (tested with local server)
- tokio runtime

## Usage Example

```
$ cargo run
AfkaraCode> edit_file src/main.rs "old" "new"
successfully edited file: successfully wrote to file
```

## Project Structure

```
afkaracode/
├── Cargo.toml          # Dependencies (openai-client, serde, tokio)
├── flake.lock          # Build lockfile
├── flake.nix           # Nix build configuration
├── src/
│   └── main.rs         # Main program and tool implementations
└── README.md           # This file
```

## Development

To run the project:

```bash
cargo new afkaracode --bin
cd afkaracode
# Replace Cargo.toml with this project's dependencies
# Copy src/main.rs from this repository
cargo run
```

The AI will interact with your files using OpenAI function calling.

## Notes

- This is a demonstration of file manipulation through AI agent tool calls
- All operations are synchronous and use Rust's std::fs functions
- The AI runs in the current directory; paths are relative to project root
