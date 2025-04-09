# PathRanger

A file system navigation enhancement tool for the command line.

PathRanger helps you navigate your file system more efficiently by automatically tracking directories you visit frequently and allowing you to quickly bookmark important locations with tags.

## Features

- Automatically tracks directories you visit most often
- Provides quick bookmarking and tagging of important directories
- Enables instant navigation to frequent locations with short commands
- Integrates with your shell to provide intelligent directory suggestions
- Search across your visited directories with fuzzy matching

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/alexandercannon/pathranger.git
cd pathranger

# Build and install
cargo install --path .
```

### Shell Integration

After installing, add the shell integration to your shell's configuration file:

For Bash (add to `~/.bashrc`):
```bash
eval "$(pathranger init --shell bash)"
```

For Zsh (add to `~/.zshrc`):
```bash
eval "$(pathranger init --shell zsh)"
```

For Fish (add to `~/.config/fish/config.fish`):
```bash
eval (pathranger init --shell fish)
```

## Usage

### Basic Commands

Mark the current directory with a tag:
```bash
pr mark notes
```

Jump to a tagged directory:
```bash
pr goto notes
```

List your most visited directories:
```bash
pr top
```

Show recently visited directories:
```bash
pr recent
```

Search across your visited directories:
```bash
pr search "project"
```

List all your tags:
```bash
pr tags
```

Remove a tag:
```bash
pr untag notes
```

Show help:
```bash
pr --help
```

### How it Works

PathRanger works by:

1. Tracking directories you visit through shell integration
2. Building a database of your navigation patterns
3. Allowing quick access to frequently used or tagged directories

The shell integration automatically records directories as you navigate with `cd`. This data is used to provide intelligent suggestions and quick access to your most used locations.

## Data Storage

PathRanger stores its database in:
- Linux: `~/.local/share/pathranger/pathranger.db`
- macOS: `~/Library/Application Support/pathranger/pathranger.db`
- Windows: `%APPDATA%\pathranger\pathranger.db`

## License

MIT