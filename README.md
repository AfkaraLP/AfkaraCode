Formatter hook

This agent automatically runs a formatter hook after every successful edit_file tool call.

How it works
- Detects the edited file's extension
- Checks ~/.config/afkaracode/config.toml for user configuration
- Picks a formatter command either from user config or built-in defaults
- Executes the command with {file} replaced by the file path

User configuration
Create a config file at:
  - Linux/macOS: $XDG_CONFIG_HOME/afkaracode/config.toml (defaults to ~/.config/afkaracode/config.toml)
  - Windows: %USERPROFILE%/.config/afkaracode/config.toml

Example config.toml:

[formatter]
# Enable or disable the formatter hook (default: true)
enable = true

# Optional custom per-extension commands. Keys are extensions without dot.
# {file} placeholder gets replaced with the file path.
[formatter.commands]
rs = "rustfmt {file}"
ts = "prettier --write {file}"
py = "ruff format {file}"

Built-in defaults
If a command is not specified in the config, sensible defaults are used based on file extension:
- rs: rustfmt
- js, jsx, ts, tsx, mjs, cjs, json, css, scss, md, markdown, yaml, yml: prettier --write
- py: ruff format
- go: gofmt -w
- sh, bash: shfmt -w
- lua: stylua
- c, h, cpp, cxx, hpp: clang-format -i
- rb: rubocop -x (auto-correct)
- php: php-cs-fixer fix

Notes
- The formatter must be installed and on PATH.
- Non-zero formatter exit codes are reported but do not stop the workflow.
- You can disable the hook globally via [formatter] enable = false.
