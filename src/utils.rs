//! This module contains every structure and enumeration in the program, as well as their
//! corresponding implementations, if any, that are not part of the core functioning of the former.
//! These include any but the [`crate::App`] structure.

use std::sync::LazyLock;

use clap::Parser;
use serde::Deserialize;
use serde::Serialize;

/// This static constant contains the message to issue to the language model as part of the system
/// prompt in the chat completion request to the OpenRouter API.
pub(crate) static LLM_INPUT: LazyLock<&str> = LazyLock::new(|| {
    "You will answer only to \"Correct\" or \"Incorrect.\" These correspond to either a\
notification that a user got a number right in a number guessing game or not, respectively. Your\
task is to, depending on whether you were notified they got it right, or not, to return a\
cowboy-like answer to the user. Make it a short text. Include just your answer and nothing more.\
Don't include emoji or otherwise non-verbal content."
});

/// This enumeration holds information about the deterministic screen states in which the use may
/// find himself while playing the game. It is mostly used for deciding what type of TUI should be
/// rendered at each point in the game.
#[derive(PartialEq)]
pub(crate) enum Screen {
    /// This variant refers to the main menu.
    MainMenu(MainMenuItem),
    /// This variant refers to the options menu.
    OptionsMenu(OptionsMenuItem),
    /// This variant refers to the state of being in-game. It thus comes accompanied of other
    /// screenful states.
    InGame(GameScreen),
    /// This variant refers to the state of being in the model menu. Even though it's not part of
    /// the menus found primarily at the start screen, it does require different rendering and thus
    /// holds its own individual screen state.
    ModelMenu,
}

/// This enumeration holds information about the different selectable items in the main menu.
#[derive(PartialEq)]
pub(crate) enum MainMenuItem {
    /// This variant refers to the option to pick "Play" in the menu, and start the game.
    Play,
    /// This variant refers to the option to pick "Options" in the menu, and enter the options menu.
    Options,
    /// This variant refers to the option to pick "Exti" in the menu, and end the game.
    Exit,
}

/// This enumeration holds information about the items to be found in the options menu.
#[derive(PartialEq)]
pub(crate) enum OptionsMenuItem {
    /// This variant refers to the option to pick "Model" in the menu, and enter the model menu
    /// screen.
    Model,
    /// This variant refers to the option to pick "Return" in the menu, and return to the previous
    /// screen.
    Return,
}

/// This enumeration holds information about the possible states in which the in-game experience may
/// be found.
#[derive(PartialEq)]
pub(crate) enum GameScreen {
    /// This variant refers to the state of being within the input prompts, inputting a range and a
    /// guess.
    Game(GameItem),
    /// This variant refers to the state of being in the end menu, with the result and a prompt to
    /// repeat for another game.
    EndMenu(EndMenuItem),
}

/// This enumeration holds information about the selectable prompts in the in-game menu.
#[derive(PartialEq)]
pub(crate) enum GameItem {
    /// This variant refers to the prompt where the user is selecting some range from which to pick
    /// a number.
    Range,
    /// This variant refers to the prompt where the user is selecting a random number within the
    /// range they selected.
    Input,
}

/// This enumeration holds information about the selectable items in the in-game end menu screen.
#[derive(PartialEq)]
pub(crate) enum EndMenuItem {
    /// This variant refers to the option to pick "Yes" in the menu, and repeat for another game.
    Repeat,
    /// This variant refers to the option to pick "No" in the menu, and exit the game.
    Exit,
}

/// This enumeration holds information about the possible result obtained by the user after guessing
/// a ranodm number, and computing one from the their input range.
#[derive(Clone, Copy)]
pub(crate) enum RandomResult {
    /// This variant represents the state of having guessed the number correctly.
    Correct,
    /// This variant represents the state of having guessed the number incorrectly.
    Incorrect,
}

/// This structure holds information about the request body to build for the chat completion request
/// to use with the OpenRouter API.
#[derive(Serialize)]
pub(crate) struct Request {
    /// This field contains the language model to be used in the request.
    model: String,
    /// This field contains the vector of messages to provide to the language model.
    messages: Vec<Message>,
}

impl Request {
    /// This function serves as a request-body builder for the chat completion request, depending on
    /// whether the request is to be made for a correct guess or otherwise an incorrect guess.
    pub(crate) fn new(model: String, result: RandomResult) -> Self {
        match result {
            RandomResult::Correct => Self {
                model,
                messages: vec![
                    Message::new(Role::System, LLM_INPUT.to_owned()),
                    Message::new(Role::User, "Correct".to_owned()),
                ],
            },
            RandomResult::Incorrect => Self {
                model,
                messages: vec![
                    Message::new(Role::System, LLM_INPUT.to_owned()),
                    Message::new(Role::User, "Incorrect".to_owned()),
                ],
            },
        }
    }
}

/// This structure holds information about the object type to use for each of the messages in the
/// chat completion request body to the OpenRouter API.
#[derive(Serialize, Deserialize)]
pub(crate) struct Message {
    /// This field refers to the role that the message is to be interpreted as coming from. LLM
    /// lingo for whose voice is this.
    role: Role,
    /// This field refers to the actual content to be used for the message; the meat of it.
    content: String,
}

impl Message {
    /// This function serves as a small utility to build messages based on a given role and a string
    /// message. It is used in the request body builder function [`Request::new`].
    const fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }

    /// This function returns the currently stored value in the [`content`] field of the structure.
    pub(crate) const fn content(&self) -> &String {
        &self.content
    }
}

/// This enumeration serves as part of the request and response body from the chat completion
/// request with the OpenRouter API.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    /// This variant represents the voice of the system prompt.
    System,
    /// This variant represents the voice of the LLM.
    Assistant,
    /// This variant represents the voice of the user.
    User,
}

/// This structure holds information about the response received as part of the chat completion
/// request to the OpenRouter API.
#[derive(Deserialize)]
pub(crate) struct ChatCompletionResponse {
    /// This field refers to the array of messages the language model may have produced in its
    /// response.
    choices: Vec<Choices>,
}

impl ChatCompletionResponse {
    /// This function returns the currently stored value in the [`choices`] field of the structure.
    pub(crate) const fn choices(&self) -> &Vec<Choices> {
        &self.choices
    }
}

/// This structure holds information about the specific dummy object used as part of the chat
/// completion request response for either one of the messages returned by the language model.
#[derive(Deserialize)]
pub(crate) struct Choices {
    /// This field refers to the actual content of the response.
    message: Message,
}

impl Choices {
    /// This function returns the currently stored value in the [`message`] field of the structure.
    pub(crate) const fn message(&self) -> &Message {
        &self.message
    }
}

/// This structure holds information about the response received as part of the model list request
/// to the OpenRouter API.
#[derive(Deserialize)]
pub(crate) struct ModelListResponse {
    /// This field contains information about the data held by the entire API model list. Within it
    /// are the details of each model.
    data: Vec<Data>,
}

impl ModelListResponse {
    /// This function returns the currently stored value for the [`data`] field of the structure.
    pub(crate) const fn data(&self) -> &Vec<Data> {
        &self.data
    }
}

/// This structure holds information about each specific model available through the OpenRouter API
/// to be received as a response to the model list request.
#[derive(Deserialize)]
pub(crate) struct Data {
    /// This field refers to the single element from the model list that this project is intered in;
    /// the codename the model receives.
    id: String,
}

impl Data {
    /// This function returns the currently stored value of the [`id`] field in the structure.
    pub(crate) const fn id(&self) -> &String {
        &self.id
    }
}

/// This enumeration holds information about the type of menu that can be rendered in a similar
/// fashion in the game. This is because the pub(crate) enum is used as a means of generalizing the behavior of
/// the actual menus in a single, more compact manner, while keeping their individual differences.
pub(crate) enum MenuType {
    /// This variant refers to the main menu in the game.
    MainMenu(u8),
    /// This variant refers to the options menu in the game.
    OptionsMenu(u8),
}

impl MenuType {
    /// This function serves as a means of returning the string representation of the enumeration.
    pub(crate) const fn repr(&self) -> &str {
        match *self {
            Self::MainMenu(_) => "Main menu",
            Self::OptionsMenu(_) => "Options menu",
        }
    }
}

/// This enumeration holds information about whether the model menu update should be performed
/// upward or downward. It is used only when updating the model menu view to determine whether the
/// command issued by the user should advance the list upward or downward.
pub(crate) enum ModelMenuDirection {
    /// This variant refers to the command of moving the viewport upward.
    Up,
    /// This variant refers to the command of moving the viewport downward.
    Down,
}

/// This enumeration holds information about whether textual input should be handled as a deletion
/// or as an addition operation to a given field.
pub(crate) enum OperationType {
    /// This variant refers to operations of addition type; adding characters to the given field.
    Addition,
    /// This variant refers to operations of deletion type; removing characters from the given
    /// field.
    Deletion,
    /// This variant refers to operations where the user switches focus between the two input
    /// prompts.
    SwitchFocus,
}

/// This structure holds information useful to the command-line argument parser in use; namely,
/// clap.
#[derive(Parser)]
#[command(name = "randy-ng", version, about, long_about = None)]
pub struct Cli {
    /// The OpenRouter model to use for the AI request.
    ///
    /// This should be set through the command-line, the environment variable or the in-game menu.
    /// If not setting it through the in-game menu, one must use the name in the OpenRouter model
    /// page that appears right below the public-facing name.
    #[arg(
        short,
        long,
        env = "OPENROUTER_MODEL",
        value_name = "MODEL_NAME",
        requires = "api_key"
    )]
    model: Option<String>,
    /// The OpenRouter API key to use for the AI request.
    ///
    /// This should be set through the command-line or the environment variable. It is required to
    /// successfully perform the chat completion request to the OpenRouter API.
    #[arg(
        long,
        env = "OPENROUTER_API_KEY",
        value_name = "YOUR_API_KEY",
        required = true
    )]
    api_key: String,
}

impl Cli {
    /// This function returns the currently stored value of the [`model`] field in the structure.
    pub(crate) const fn model(&self) -> Option<&String> {
        self.model.as_ref()
    }

    /// This function returns the currently stored value of the [`api_key`] field in the structure.
    pub(crate) const fn api_key(&self) -> &String {
        &self.api_key
    }
}
