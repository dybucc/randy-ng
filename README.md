# Randy-NG

A new-generation terminal-based number guessing game with AI-powered cowboy responses.

## Description

Randy-NG is a modern TUI (Terminal User Interface) implementation of the classic number guessing
game. What makes it special is the integration with AI language models through the OpenRouter API,
which provides cowboy-themed responses to your guesses, making the experience more engaging and
entertaining.

## Features

- 🎮 **Interactive TUI**: Clean, intuitive terminal interface built with Ratatui
- 🤖 **AI Integration**: Get cowboy-style responses from various language models via OpenRouter API
- 🎯 **Flexible Gameplay**: Choose your own number ranges for each game
- 📊 **Score Tracking**: Keep track of your correct guesses across multiple rounds
- 🔧 **Model Selection**: Choose from dozens of available language models
- ⌨️ **Vim-like Navigation**: Familiar j/k/h/l key bindings for navigation
- 🎨 **Modern Interface**: Responsive design with proper error handling and visual feedback

## Installation

### Prerequisites

- Rust 1.70 or later
- An OpenRouter API key (get one at [openrouter.ai](https://openrouter.ai))

### From Source

```bash
git clone <repository-url>
cd randy-ng
cargo build --release
```

## Usage

### Basic Usage

```bash
# Set your API key as an environment variable
export OPENROUTER_API_KEY="your_api_key_here"

# Run the game
./target/release/randy-ng
```

### Command Line Options

```bash
# Specify a model directly
randy-ng --model "qwen/qwen3-32b:free" --api-key "your_key"

# Using short flags
randy-ng -m "anthropic/claude-3-haiku" --api-key "your_key"
```

### Environment Variables

You can set these environment variables to avoid passing them as arguments:

- `OPENROUTER_API_KEY`: Your OpenRouter API key (required)
- `OPENROUTER_MODEL`: Default model to use (optional)

## How to Play

1. **Start the Game**: Launch the application and select "Play" from the main menu
2. **Set Range**: Enter a number range in the format `n..m` (e.g., `1..100`)
3. **Make Guess**: Enter your guess within the specified range
4. **Get Response**: Receive an AI-generated cowboy response based on whether you're right or wrong
5. **Continue**: Choose to play another round or exit

### Controls

- **j**: Move down / Navigate down in menus
- **k**: Move up / Navigate up in menus
- **l**: Select / Enter / Move right
- **h**: Go back / Move left
- **Tab**: Switch between input fields (during gameplay)
- **Enter**: Submit input / Confirm selection
- **Backspace**: Delete characters in input fields
- **q**: Quit the application

## Configuration

### Model Selection

You can choose from various language models:

1. Go to **Options** → **Model** from the main menu
2. Browse the list of available models using j/k
3. Press 'l' to select a model
4. The selected model will be used for future AI responses

### API Key Setup

The easiest way to set up your API key is through environment variables:

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export OPENROUTER_API_KEY="your_actual_api_key_here"
```

Alternatively, you can pass it as a command-line argument each time:

```bash
randy-ng --api-key "your_key_here"
```

## Error Handling

The application provides clear error messages for common issues:

- **400**: Bad request - Check your input format
- **401**: Invalid credentials - Verify your API key
- **402**: Insufficient credits - Add credits to your OpenRouter account
- **403**: Flagged input - Your input was flagged by content filters
- **408**: Timed out - Request took too long, try again
- **429**: Rate limited - Wait a moment before making another request
- **502**: Invalid response or model down - Try a different model
- **503**: No available providers - Service temporarily unavailable

## Development

### Building from Source

```bash
cargo build
```

## Dependencies

- **clap**: Command-line argument parsing
- **color-eyre**: Enhanced error reporting
- **fastrand**: Random number generation
- **ratatui**: Terminal user interface framework
- **regex**: Input validation
- **serde**: Serialization/deserialization
- **ureq**: HTTP client for API requests

## License

This project is released into the public domain under the Unlicense. See the project files for details.

## Contributing

Contributions are welcome! Please feel free to submit issues, feature requests, or pull requests.

## Acknowledgments

- Built with [Ratatui](https://ratatui.rs/) for the terminal interface
- Powered by [OpenRouter](https://openrouter.ai/) for AI model access
- Inspired by classic number guessing games with a modern twist
