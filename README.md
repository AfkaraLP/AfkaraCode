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
- Lua runtime for user-defined tools at runtime:
  - Drop .lua files in:
    - Local: ./lua_tools
    - XDG config: $XDG_CONFIG_HOME/afkaracode/plugins and each $XDG_CONFIG_DIRS/afkaracode/plugins
  - Lua file returns a table with fields: name, description, entry (function name), args, and the entry function
  - The tool's description from the Lua file is propagated and used when registering the tool
  - Arg types supported in args: string, number, bool (case-insensitive: bool/boolean)
  - Built-in `http` module with http.get/post/request for network requests

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
- On Linux/macOS, XDG base directories are honored for plugins:
  - $XDG_CONFIG_HOME/afkaracode/plugins
  - Each $XDG_CONFIG_DIRS/afkaracode/plugins

## Usage Example

```
$ cargo run
AfkaraCode> edit_file src/main.rs "old" "new"
successfully edited file: successfully wrote to file
```

### Define a Lua tool
Create a file at lua_tools/http_example.lua (or $XDG_CONFIG_HOME/afkaracode/plugins/http_example.lua) with:

```
return {
  name = "http_example",
  description = "Example Lua tool that fetches a URL and returns first 200 chars.",
  entry = "run",
  args = {
    { name = "url", description = "URL to fetch", type = "string", required = true }
  },
  run = function(args)
    local body = http.get(args.url)
    if not body then return "" end
    return string.sub(body, 1, 200)
  end
}
```

Restart the app, and the tool will be registered automatically as `http_example`.

Supported arg types in Lua tools
- string: default if type is omitted or unrecognized
- number: numeric arguments
- bool/boolean: boolean arguments

Tool description propagation
- The description field provided in the Lua tool table is now used as the tool's description when registered, so it appears to the agent and users.

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
