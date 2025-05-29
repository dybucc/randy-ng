//! This crate contains all the code, temporarily organized as a single binary, for the game randy.
//! It shouldn't be used outside the game as it's pretty much unusable beyond it.

#![expect(
    clippy::cargo_common_metadata,
    reason = "Temporary during development prior to crates.io publishing"
)]
#![expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Temporary allow during development."
)]

use std::{rc::Rc, sync::LazyLock, time::Duration};

use clap::Parser;
use color_eyre::{
    eyre::{eyre, Result},
    install,
};
use fastrand::Rng;
use ratatui::{
    crossterm::event::{poll, read, Event, KeyCode},
    init,
    layout::Flex,
    prelude::{
        Alignment, Buffer, Color, Constraint, Layout, Line, Modifier, Rect, Style, Text, Widget,
    },
    restore,
    symbols::{block::FULL, DOT},
    widgets::{Block, BorderType, Borders, Clear},
    DefaultTerminal,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use ureq::{agent, Error};

fn main() -> Result<()> {
    install()?;
    let _ = Cli::parse();

    let terminal = init();
    let result = App::default().run(terminal);
    restore();

    match result.err() {
        None => Ok(()),
        Some(err) => match err.downcast::<Error>() {
            Ok(err) => match err {
                Error::StatusCode(status) => match status {
                    400 => Err(eyre!("bad request")),
                    401 => Err(eyre!("invalid credentials")),
                    402 => Err(eyre!("insufficient credits")),
                    403 => Err(eyre!("flagged input")),
                    408 => Err(eyre!("timed out")),
                    429 => Err(eyre!("rate limited")),
                    502 => Err(eyre!("invalid response or model down")),
                    503 => Err(eyre!("no available providers")),
                    _ => Err(eyre!("unknown error")),
                },
                _ => Err(eyre!("unknown error")),
            },
            Err(_) => Ok(()),
        },
    }
}

/// This structure holds information useful to the command-line argument parser in use; namely,
/// clap.
#[derive(Parser)]
#[command(name = "randy-ng", version, about, long_about = None)]
struct Cli {
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

/// This enumeration holds information about the deterministic screen states in which the use may
/// find himself while playing the game. It is mostly used for deciding what type of TUI should be
/// rendered at each point in the game.
#[derive(PartialEq)]
enum Screen {
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
enum MainMenuItem {
    /// This variant refers to the option to pick "Play" in the menu, and start the game.
    Play,
    /// This variant refers to the option to pick "Options" in the menu, and enter the options menu.
    Options,
    /// This variant refers to the option to pick "Exti" in the menu, and end the game.
    Exit,
}

/// This enumeration holds information about the items to be found in the options menu.
#[derive(PartialEq)]
enum OptionsMenuItem {
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
enum GameScreen {
    /// This variant refers to the state of being within the input prompts, inputting a range and a
    /// guess.
    Game(GameItem),
    /// This variant refers to the state of being in the end menu, with the result and a prompt to
    /// repeat for another game.
    EndMenu(EndMenuItem),
}

/// This enumeration holds information about the selectable prompts in the in-game menu.
#[derive(PartialEq)]
enum GameItem {
    /// This variant refers to the prompt where the user is selecting some range from which to pick
    /// a number.
    Range,
    /// This variant refers to the prompt where the user is selecting a random number within the
    /// range they selected.
    Input,
}

/// This enumeration holds information about the selectable items in the in-game end menu screen.
#[derive(PartialEq)]
enum EndMenuItem {
    /// This variant refers to the option to pick "Yes" in the menu, and repeat for another game.
    Repeat,
    /// This variant refers to the option to pick "No" in the menu, and exit the game.
    Exit,
}

/// This enumeration holds information about the possible result obtained by the user after guessing
/// a ranodm number, and computing one from the their input range.
#[derive(Clone, Copy)]
enum RandomResult {
    /// This variant represents the state of having guessed the number correctly.
    Correct,
    /// This variant represents the state of having guessed the number incorrectly.
    Incorrect,
}

static LLM_INPUT: LazyLock<&str> = LazyLock::new(|| {
    "You will answer only to \"Correct\" or \"Incorrect.\" These correspond to either a\
notification that a user got a number right in a number guessing game or not, respectively. Your\
task is to, depending on whether you were notified they got it right, or not, to return a\
cowboy-like answer to the user. Make it a short text. Include just your answer and nothing more.\
Don't include emoji or otherwise non-verbal content."
});

/// This structure holds information about the request body to build for the chat completion request
/// to use with the OpenRouter API.
#[derive(Serialize)]
struct Request {
    /// This field contains the language model to be used in the request.
    model: String,
    /// This field contains the vector of messages to provide to the language model.
    messages: Vec<Message>,
}

impl Request {
    /// This function serves as a request-body builder for the chat completion request, depending on
    /// whether the request is to be made for a correct guess or otherwise an incorrect guess.
    fn new(model: String, result: RandomResult) -> Self {
        match result {
            RandomResult::Correct => Self {
                model,
                messages: vec![
                    Message::new(Role::System, LLM_INPUT.to_string()),
                    Message::new(Role::User, "Correct".to_owned()),
                ],
            },
            RandomResult::Incorrect => Self {
                model,
                messages: vec![
                    Message::new(Role::System, LLM_INPUT.to_string()),
                    Message::new(Role::User, "Incorrect".to_owned()),
                ],
            },
        }
    }
}

/// This structure holds information about the object type to use for each of the messages in the
/// chat completion request body to the OpenRouter API.
#[derive(Serialize, Deserialize)]
struct Message {
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
}

/// This enumeration serves as part of the request and response body from the chat completion
/// request with the OpenRouter API.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
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
struct ChatCompletionResponse {
    /// This field refers to the array of messages the language model may have produced in its
    /// response.
    choices: Vec<Choices>,
}

/// This structure holds information about the specific dummy object used as part of the chat
/// completion request response for either one of the messages returned by the language model.
#[derive(Deserialize)]
struct Choices {
    /// This field refers to the actual content of the response.
    message: Message,
}

/// This structure holds information about the response received as part of the model list request
/// to the OpenRouter API.
#[derive(Deserialize)]
struct ModelListResponse {
    /// This field contains information about the data held by the entire API model list. Within it
    /// are the details of each model.
    data: Vec<Data>,
}

/// This structure holds information about each specific model available through the OpenRouter API
/// to be received as a response to the model list request.
#[derive(Deserialize)]
struct Data {
    /// This field refers to the single element from the model list that this project is intered in;
    /// the codename the model receives.
    id: String,
}

/// This enumeration holds information about the type of menu that can be rendered in a similar
/// fashion in the game. This is because the enum is used as a means of generalizing the behavior of
/// the actual menus in a single, more compact manner, while keeping their individual differences.
enum MenuType {
    /// This variant refers to the main menu in the game.
    MainMenu(u8),
    /// This variant refers to the options menu in the game.
    OptionsMenu(u8),
}

impl MenuType {
    /// This function serves as a means of returning the string representation of the enumeration.
    const fn repr(&self) -> &str {
        match *self {
            Self::MainMenu(_) => "Main menu",
            Self::OptionsMenu(_) => "Options menu",
        }
    }
}

/// This enumeration holds information about whether the model menu update should be performed
/// upward or downward. It is used only when updating the model menu view to determine whether the
/// command issued by the user should advance the list upward or downward.
enum ModelMenuDirection {
    /// This variant refers to the command of moving the viewport upward.
    Up,
    /// This variant refers to the command of moving the viewport downward.
    Down,
}

/// This enumeration holds information about whether textual input should be handled as a deletion
/// or as an addition operation to a given field.
enum OperationType {
    /// This variant refers to operations of addition type; adding characters to the given field.
    Addition,
    /// This variant refers to operations of deletion type; removing characters from the given
    /// field.
    Deletion,
    /// This variant refers to operations where the user switches focus between the two input
    /// prompts.
    SwitchFocus,
}

/// This structure holds information about the application itself, keeping inside it both state and
/// functions relative to the drawing and updating of the state.
struct App<'line> {
    /// This field refers to the condition of the game being run.
    exit: bool,
    /// This field refers to the current screen in which the user finds himself, generally as a
    /// consequence of a prior keypress.
    screen: Screen,
    /// This field refers to the score accumulated by the user when playing multiple games in a row.
    score: u8,
    /// This field refers to the ranged input taken from the user during the in-game experience.
    range_input: String,
    /// This field refers to the regular guess input taken from the user during the in-game
    /// experience.
    input: String,
    /// This field refers to the result of having computed the guess of the user within the given
    /// range and thus having determined whether they are right or wrong. This may not be
    /// initialized until a game is actually played, so it's wrapped in an `Option`.
    result: Option<RandomResult>,
    /// This field refers to the model selected by the user to process the request to the make to
    /// the OpenRouter API for chat completion.
    model: String,
    /// This field refers to the complete set of models retrieved from the OpenRouter API which are
    /// available for use in the menu.
    models: Vec<String>,
    /// This field refers to the set of models that are currently in display within the viewport of
    /// the TUI. This is part of the persistent state required for the scrolling feature.
    models_view: Vec<Line<'line>>,
    /// This field refers to the set of selectors / spaces to display which model is currently
    /// selected to be used. This is part of the persistent state required for the scrolling
    /// feature.
    selectors_view: Vec<Line<'line>>,
    /// This field refers to the currently selected model in the viewport. This is part of the
    /// persistent state required for the scrolling feature.
    model_view_selected: String,
    /// This field refers to the offset by which the first element of the viewport is not seen
    /// anymore. This is core to the scrolling feature and is thus part of the persistent state.
    model_view_offset: u16,
    /// This field refers to the API key to be used when performing the chat completion request to
    /// the OpenRouter API.
    api_key: String,
    /// This field refers to the regular expression in use to validate the input of the user in the
    /// ranged numbers prompt.
    ranged_re: Regex,
    /// This field refers to the regular expression in use to validate the input of the user in the
    /// regular guess number prompt.
    input_re: Regex,
    /// This field refers to the flag that allows informing the user their input is invalid.
    extra_line_help: bool,
    /// This field refers to the flag that allows informing the user the request is being processed.
    processing_request: bool,
    /// This field refers to the RNG to be used when the user's input is processed and the result of
    /// their guess is computed.
    rng: Rng,
    /// This field refers to the output of the chat completion request, holding only the message
    /// retrieved from the language model's response.
    chat_completion_output: String,
}

impl Default for App<'_> {
    fn default() -> Self {
        let cli = Cli::parse();

        Self {
            exit: false,
            screen: Screen::MainMenu(MainMenuItem::Play),
            score: 0,
            result: None,
            range_input: String::new(),
            input: String::new(),
            model: cli
                .model
                .unwrap_or_else(|| "qwen/qwen3-32b:free".to_owned()),
            models: Vec::new(),
            models_view: Vec::new(),
            selectors_view: Vec::new(),
            model_view_selected: String::new(),
            model_view_offset: 0,
            api_key: cli.api_key,
            ranged_re: Regex::new(r"\A\d+\.\.\d+\z").expect("bad regex syntax"),
            input_re: Regex::new(r"\A\d+\z").expect("bad regex syntax"),
            extra_line_help: false,
            processing_request: false,
            rng: Rng::new(),
            chat_completion_output: String::new(),
        }
    }
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match &self.screen {
            Screen::MainMenu(screen) => {
                App::main_menu(area, buf, screen);
            }
            Screen::OptionsMenu(screen) => {
                App::options_menu(area, buf, screen);
            }
            Screen::InGame(screen) => match screen {
                GameScreen::Game(screen) => self.take_input(area, buf, screen),
                GameScreen::EndMenu(screen) => self.end_menu(area, buf, screen),
            },
            Screen::ModelMenu => self.model_menu(area, buf),
        };
    }
}

impl App<'_> {
    /// This function serves as a way of fetching the models currently available for use through the
    /// OpenRouter API. Note it does not require any type of authentication.
    fn fetch_models(&mut self) {
        let response: ModelListResponse = ureq::get("https://openrouter.ai/api/v1/models")
            .call()
            .expect("models request failed")
            .into_body()
            .read_json()
            .expect("json failed to parse");

        for model in response.data {
            self.models.push(model.id);
        }
    }

    /// This function serves as a means of validating user input for the range and guess.
    fn validate_input(&self) -> bool {
        if self.ranged_re.is_match(&self.range_input) && self.input_re.is_match(&self.input) {
            // process the ranged input
            let (start, end) = self.range_input.split_at(
                self.range_input
                    .find("..")
                    .expect("validate_input parsing failed"),
            );
            let end: String = end.chars().rev().collect();
            let (end, _) = end.split_at(end.find("..").expect("validate_input parsing failed"));
            let start: usize = start.parse().expect("validate_input parsing failed");
            let end: usize = end.parse().expect("validate_input parsing failed");
            let flag1 = start < end;

            // process the guess input
            let guess: usize = self.input.parse().expect("validate_input parsing failed");
            let flag2 = guess >= start && guess <= end;

            return flag1 && flag2;
        }

        false
    }

    /// This function processes a random number in the range given by the user and stores the result
    /// in the corresponding internal state of the application.
    fn process_random(&mut self) {
        let (start, end) = self.range_input.split_at(
            self.range_input
                .find("..")
                .expect("process_random parsing failed"),
        );
        let end: String = end.chars().rev().collect();
        let (end, _) = end.split_at(end.find("..").expect("process_random parsing failed"));

        let start: usize = start.parse().expect("process_random parsing failed");
        let end: usize = end.parse().expect("process_random parsing failed");
        let guess: usize = self.input.parse().expect("process_random parsing failed");

        let random = self.rng.usize(start..=end);

        if guess == random {
            self.result = Some(RandomResult::Correct);
        } else {
            self.result = Some(RandomResult::Incorrect);
        }
    }

    /// This function processes a chat completion request of the OpenRouter API, and retrieves the
    /// message returned by the language model if the request doesn't error out. The output is then
    /// stored in the application's persistent state.
    #[expect(
        clippy::unwrap_in_result,
        reason = "The expects are used on Option<> values, which are not compatible with Result<> function return values"
    )]
    fn process_request(&mut self) -> Result<()> {
        let request_body = Request::new(
            self.model.clone(),
            self.result.expect("result not processed yet"),
        );
        let agent = agent();

        loop {
            match agent
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send_json(&request_body)
            {
                Ok(response) => {
                    let response: ChatCompletionResponse = response.into_body().read_json()?;
                    let output = response
                        .choices
                        .last()
                        .expect("empty vector when processing request")
                        .message
                        .content
                        .clone();

                    if output.is_empty() {
                        continue;
                    }
                    self.chat_completion_output = output;
                    break Ok(());
                }
                Err(err) => break Err(err.into()),
            }
        }
    }

    /// This function serves as a means of running the application by making use of TUI callbacks
    /// and a event handling functionality.
    fn run(&mut self, mut term: DefaultTerminal) -> Result<()> {
        while !self.exit {
            let _ = term.draw(|frame| frame.render_widget(&mut *self, frame.area()))?;
            self.handle_events()?;
        }
        Ok(())
    }

    /// This function handles the event where the program requires the chat completion request to be
    /// processed.
    fn handle_request(&mut self) -> Result<()> {
        if self.processing_request {
            self.process_random();
            self.process_request()?;
            self.screen = Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat));
            self.processing_request = false;
        }
        Ok(())
    }

    /// This function handles updates to the model menu viewport. It gets issued a command to update
    /// in either one of of the upward or downward directions, and makes the corresponding changes
    /// to the persistent state related to this part of the application.
    fn handle_model_menu_updates(&mut self, direction: ModelMenuDirection) {
        match direction {
            ModelMenuDirection::Down => {
                let mut first_model_after_view = String::new();

                for (idx, model) in self.models.iter().enumerate() {
                    if *model == self.model_view_selected
                        && model
                            != self
                                .models
                                .last()
                                .expect("empty vector when browsing down models")
                    {
                        if self.model_view_selected
                            == self
                                .models_view
                                .last()
                                .expect("empty vector when browsing down models")
                                .to_string()
                        {
                            first_model_after_view.clone_from(
                                self.models
                                    .get(idx + 1)
                                    .expect("item not found when browsing down models"),
                            );
                        }

                        self.model_view_selected.clone_from(
                            self.models
                                .get(idx + 1)
                                .expect("item not found when browsing down models"),
                        );
                        break;
                    }
                }

                if !first_model_after_view.is_empty()
                    && self.model_view_selected == first_model_after_view
                {
                    self.model_view_offset += 1;
                }
            }
            ModelMenuDirection::Up => {
                let mut first_model_before_view = String::new();

                for (idx, model) in self.models.iter().enumerate() {
                    if *model == self.model_view_selected
                        && model
                            != self
                                .models
                                .first()
                                .expect("empty vector when browsing up models")
                    {
                        if self.model_view_selected
                            == self
                                .models_view
                                .first()
                                .expect("empty vect when browsing up models")
                                .to_string()
                        {
                            first_model_before_view.clone_from(
                                self.models
                                    .get(idx - 1)
                                    .expect("item not found while browsing up models"),
                            );
                        }

                        self.model_view_selected.clone_from(
                            self.models
                                .get(idx - 1)
                                .expect("item not found while browsing up models"),
                        );
                        break;
                    }
                }

                if !first_model_before_view.is_empty()
                    && self.model_view_selected == first_model_before_view
                {
                    self.model_view_offset -= 1;
                }
            }
        }
    }

    /// This function serves as a textual input hanlder when the user is either inputting or
    /// deleting characters on the in-game input prompts.
    fn handle_textual_input(&mut self, operation: OperationType, char: Option<char>) {
        match &self.screen {
            Screen::InGame(GameScreen::Game(GameItem::Range)) => match operation {
                OperationType::Addition => {
                    self.range_input.push(char.expect("no character to push"));
                }
                OperationType::Deletion => {
                    let _ = self.range_input.pop();
                }
                OperationType::SwitchFocus => {
                    self.screen = Screen::InGame(GameScreen::Game(GameItem::Input));
                }
            },
            Screen::InGame(GameScreen::Game(GameItem::Input)) => match operation {
                OperationType::Addition => {
                    self.input.push(char.expect("no character to push"));
                }
                OperationType::Deletion => {
                    let _ = self.input.pop();
                }
                OperationType::SwitchFocus => {
                    self.screen = Screen::InGame(GameScreen::Game(GameItem::Range));
                }
            },
            _ => {}
        }
    }

    /// This function holds the event handling behavior corresponding to the 'l' character press
    /// event.
    fn handle_l_input(&mut self) {
        match &self.screen {
            Screen::MainMenu(MainMenuItem::Play) => {
                self.screen = Screen::InGame(GameScreen::Game(GameItem::Range));
            }
            Screen::MainMenu(MainMenuItem::Options) => {
                self.screen = Screen::OptionsMenu(OptionsMenuItem::Model);
            }
            Screen::MainMenu(MainMenuItem::Exit) => self.exit = true,
            Screen::OptionsMenu(OptionsMenuItem::Model) => {
                self.screen = Screen::ModelMenu;

                self.model_view_offset = 0;
                self.fetch_models();
                self.model_view_selected = self
                    .models
                    .first()
                    .expect("empty vector while assigning selected model")
                    .to_owned();
            }
            Screen::OptionsMenu(OptionsMenuItem::Return) => {
                self.screen = Screen::MainMenu(MainMenuItem::Play);
            }
            Screen::ModelMenu => {
                self.model = self.model_view_selected.clone();
            }
            Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat)) => {
                self.screen = Screen::InGame(GameScreen::Game(GameItem::Range));
            }
            Screen::InGame(GameScreen::EndMenu(EndMenuItem::Exit)) => {
                self.exit = true;
            }
            _ => {}
        }
    }

    /// This function holds the event handling behavior corresponding to the 'k' character press
    /// event.
    fn handle_k_input(&mut self) {
        match &self.screen {
            Screen::MainMenu(MainMenuItem::Exit) => {
                self.screen = Screen::MainMenu(MainMenuItem::Options);
            }
            Screen::MainMenu(MainMenuItem::Options) => {
                self.screen = Screen::MainMenu(MainMenuItem::Play);
            }
            Screen::OptionsMenu(OptionsMenuItem::Return) => {
                self.screen = Screen::OptionsMenu(OptionsMenuItem::Model);
            }
            Screen::ModelMenu => {
                self.handle_model_menu_updates(ModelMenuDirection::Up);
            }
            Screen::InGame(GameScreen::EndMenu(EndMenuItem::Exit)) => {
                self.screen = Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat));
            }
            _ => {}
        }
    }

    /// This function holds the event handling behavior corresponding to the 'j' character press
    /// event.
    fn handle_j_input(&mut self) {
        match &self.screen {
            Screen::MainMenu(MainMenuItem::Play) => {
                self.screen = Screen::MainMenu(MainMenuItem::Options);
            }
            Screen::MainMenu(MainMenuItem::Options) => {
                self.screen = Screen::MainMenu(MainMenuItem::Exit);
            }
            Screen::OptionsMenu(OptionsMenuItem::Model) => {
                self.screen = Screen::OptionsMenu(OptionsMenuItem::Return);
            }
            Screen::ModelMenu => {
                self.handle_model_menu_updates(ModelMenuDirection::Down);
            }
            Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat)) => {
                self.screen = Screen::InGame(GameScreen::EndMenu(EndMenuItem::Exit));
            }
            _ => {}
        }
    }

    /// This function holds the event handling behavior corresponding to the 'h' character press
    /// event.
    fn handle_h_input(&mut self) {
        if matches!(&self.screen, Screen::ModelMenu) {
            self.screen = Screen::OptionsMenu(OptionsMenuItem::Model);
        }
    }

    /// This function serves mostly as an input handling mechanism, and as a means of processing the
    /// chat completion request with the OpenRouter API.
    fn handle_events(&mut self) -> Result<()> {
        self.handle_request()?;

        if poll(Duration::from_millis(100)).is_ok_and(|value| value) {
            if let Event::Key(key) = read()? {
                match key.code {
                    KeyCode::Char(ch)
                        if matches!(self.screen, Screen::InGame(GameScreen::Game(_)))
                            && !self.processing_request =>
                    {
                        self.handle_textual_input(OperationType::Addition, Some(ch));
                    }
                    KeyCode::Tab
                        if matches!(self.screen, Screen::InGame(GameScreen::Game(_)))
                            && !self.processing_request =>
                    {
                        self.handle_textual_input(OperationType::SwitchFocus, None);
                    }
                    KeyCode::Backspace
                        if matches!(self.screen, Screen::InGame(GameScreen::Game(_)))
                            && !self.processing_request =>
                    {
                        self.handle_textual_input(OperationType::Deletion, None);
                    }
                    KeyCode::Enter
                        if matches!(self.screen, Screen::InGame(GameScreen::Game(_)))
                            && !self.processing_request =>
                    {
                        if self.validate_input() {
                            self.extra_line_help = false;
                            self.processing_request = true;
                        } else {
                            self.extra_line_help = true;
                        }
                    }
                    KeyCode::Char('q') => self.exit = true,
                    KeyCode::Char('j') => self.handle_j_input(),
                    KeyCode::Char('k') => self.handle_k_input(),
                    KeyCode::Char('l') => self.handle_l_input(),
                    KeyCode::Char('h') => self.handle_h_input(),
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// This function is a shorthand way of clearing a given area in the given buffer by rendering a
    /// special widget on that area.
    fn clear(area: Rect, buf: &mut Buffer) {
        let clear = Clear;
        clear.render(area, buf);
    }

    /// This function initializes the screen area and the block to be used when rendering generic
    /// menus. Generic menus are denoted by those with a similar appearance. Currently, only the
    /// main menu and the options menu are considered generic.
    #[expect(
        clippy::indexing_slicing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    fn init_menu(area: Rect, buf: &mut Buffer, menu: MenuType) -> Rc<[Rect]> {
        let screen = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(area);
        let item_count = match menu {
            MenuType::MainMenu(num) => num,
            MenuType::OptionsMenu(num) => num,
        };

        let block_space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(screen[1])[1];
        let block_layout = Layout::vertical([Constraint::Max((item_count + 2).into())])
            .flex(Flex::Center)
            .split(block_space)[0];
        let block = Block::bordered()
            .title_top(menu.repr())
            .title_bottom("(j) down / (k) up / (l) select")
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .border_type(BorderType::Rounded);

        let item_space = block.inner(block_layout);

        block.render(block_layout, buf);

        Layout::vertical(vec![Constraint::Max(1); item_count.into()]).split(item_space)
    }

    /// This function renders the main menu screen.
    #[expect(
        clippy::indexing_slicing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    #[expect(
        clippy::missing_asserts_for_indexing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    fn main_menu(area: Rect, buf: &mut Buffer, screen: &MainMenuItem) {
        Self::clear(area, buf);

        let item_layout = Self::init_menu(area, buf, MenuType::MainMenu(3));

        let content_style = Style::default().fg(Color::White);
        let active_content_style = content_style.bg(Color::Green);

        let mut items = [
            Line::raw("Play").centered(),
            Line::raw("Options").centered(),
            Line::raw("Exit").centered(),
        ];
        match screen {
            MainMenuItem::Play => {
                items[0] = items[0].clone().style(active_content_style);
                items[1] = items[1].clone().style(content_style);
                items[2] = items[2].clone().style(content_style);
            }
            MainMenuItem::Options => {
                items[0] = items[0].clone().style(content_style);
                items[1] = items[1].clone().style(active_content_style);
                items[2] = items[2].clone().style(content_style);
            }
            MainMenuItem::Exit => {
                items[0] = items[0].clone().style(content_style);
                items[1] = items[1].clone().style(content_style);
                items[2] = items[2].clone().style(active_content_style);
            }
        }

        items[0].clone().render(item_layout[0], buf);
        items[1].clone().render(item_layout[1], buf);
        items[2].clone().render(item_layout[2], buf);
    }

    /// This function renders the options menu.
    #[expect(
        clippy::indexing_slicing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    #[expect(
        clippy::missing_asserts_for_indexing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    fn options_menu(area: Rect, buf: &mut Buffer, screen: &OptionsMenuItem) {
        Self::clear(area, buf);

        let item_layout = Self::init_menu(area, buf, MenuType::OptionsMenu(2));

        let content_style = Style::default().fg(Color::White);
        let active_content_style = content_style.bg(Color::Green);

        let mut items = [
            Line::raw("Model").centered(),
            Line::raw("Return").centered(),
        ];
        match screen {
            OptionsMenuItem::Model => {
                items[0] = items[0].clone().style(active_content_style);
                items[1] = items[1].clone().style(content_style);
            }
            OptionsMenuItem::Return => {
                items[0] = items[0].clone().style(content_style);
                items[1] = items[1].clone().style(active_content_style);
            }
        }

        items[0].clone().render(item_layout[0], buf);
        items[1].clone().render(item_layout[1], buf);
    }

    /// This function renders the model menu.
    #[expect(
        clippy::indexing_slicing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    #[expect(
        clippy::missing_asserts_for_indexing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    fn model_menu(&mut self, area: Rect, buf: &mut Buffer) {
        Self::clear(area, buf);

        let space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(area)[1];
        let space = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Percentage(100),
            Constraint::Percentage(30),
        ])
        .split(space)[1];

        let model_list_block = Block::bordered()
            .title_top("Model list")
            .title_bottom(Line::raw("(j) down / (k) up / (l) select / (h) return"))
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green))
            .border_type(BorderType::Rounded);
        let list_space = model_list_block.inner(space);
        let list_space =
            Layout::horizontal([Constraint::Percentage(5), Constraint::Percentage(95)])
                .split(list_space);
        let selector_space = list_space[0];
        let model_space = list_space[1];

        let selector_space_layout =
            Layout::vertical(vec![Constraint::Max(1); selector_space.height as usize])
                .split(selector_space);
        let model_space_layout =
            Layout::vertical(vec![Constraint::Max(1); model_space.height as usize])
                .split(model_space);

        model_list_block.render(space, buf);

        let content_style = Style::default().fg(Color::White);
        let active_content_style = content_style.bg(Color::Green);

        self.models_view.clear();
        self.selectors_view.clear();
        for model in self.models.iter().skip(self.model_view_offset as usize) {
            if *model == self.model_view_selected {
                if *model == self.model {
                    self.selectors_view
                        .push(Line::styled(DOT, active_content_style).alignment(Alignment::Center));
                } else {
                    self.selectors_view
                        .push(Line::styled(" ", active_content_style));
                }
                self.models_view
                    .push(Line::styled(model.to_owned(), active_content_style));
            } else {
                if *model == self.model {
                    self.selectors_view
                        .push(Line::styled(DOT, content_style).alignment(Alignment::Center));
                } else {
                    self.selectors_view.push(Line::styled(" ", content_style));
                }
                self.models_view
                    .push(Line::styled(model.to_owned(), content_style));
            }
        }
        self.models_view.truncate(model_space.height as usize);
        self.selectors_view.truncate(selector_space.height as usize);

        for (idx, model) in self.models_view.iter().enumerate() {
            model.render(model_space_layout[idx], buf);
        }
        for (idx, selector) in self.selectors_view.iter().enumerate() {
            selector.render(selector_space_layout[idx], buf);
        }
    }

    /// This function renders the prompts to take ranged input and regular guess input from the
    /// user.
    #[expect(
        clippy::indexing_slicing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    #[expect(
        clippy::missing_asserts_for_indexing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    fn take_input(&self, area: Rect, buf: &mut Buffer, screen: &GameItem) {
        Self::clear(area, buf);

        let space = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(area)[1];
        let space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(space)[1];

        let layout = if self.extra_line_help || self.processing_request {
            Layout::vertical([Constraint::Max(3), Constraint::Max(3), Constraint::Max(1)])
                .flex(Flex::Center)
                .split(space)
        } else {
            Layout::vertical([Constraint::Max(3), Constraint::Max(3)])
                .flex(Flex::Center)
                .split(space)
        };

        let ranged_input_block = Block::bordered()
            .title_top("Input a range in the format n..m where n < m")
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green))
            .border_type(BorderType::Rounded);
        let guess_input_block = Block::bordered()
            .title_top("Input a number in the above range")
            .title_bottom("(tab) switch between panels / (ret) continue")
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green))
            .border_type(BorderType::Rounded);
        if self.extra_line_help {
            let help_line = Block::new()
                .title_top("Incorrect input")
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .title_alignment(Alignment::Center)
                .borders(Borders::TOP);

            help_line.render(layout[2], buf);
        } else if self.processing_request {
            let processing_text = Block::new()
                .title_top(format!(" {DOT} Processing {DOT} "))
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::White))
                .borders(Borders::TOP);

            processing_text.render(layout[2], buf);
        }

        let ranged_input_space = ranged_input_block.inner(layout[0]);
        let guess_input_space = guess_input_block.inner(layout[1]);

        ranged_input_block.render(layout[0], buf);
        guess_input_block.render(layout[1], buf);

        let mut ranged_input =
            Line::styled(self.range_input.clone(), Style::default().fg(Color::White))
                .alignment(Alignment::Center);
        let mut input = Line::styled(self.input.clone(), Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        match screen {
            GameItem::Range => {
                ranged_input.push_span(FULL);
            }
            GameItem::Input => {
                input.push_span(FULL);
            }
        }

        ranged_input.render(ranged_input_space, buf);
        input.render(guess_input_space, buf);
    }

    /// This function renders the end game menu, as well as the prompt to continue.
    #[expect(
        clippy::indexing_slicing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    #[expect(
        clippy::missing_asserts_for_indexing,
        reason = "The collection is created in place with a small amount of elements of known index."
    )]
    fn end_menu(&self, area: Rect, buf: &mut Buffer, screen: &EndMenuItem) {
        Self::clear(area, buf);

        let space = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(area)[1];
        let space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(space)[1];

        let layout = Layout::vertical([Constraint::Min(1), Constraint::Max(4)])
            .flex(Flex::Center)
            .split(space);

        let result_block = Block::bordered()
            .title_top({
                match self.result.expect("result not yet computed") {
                    RandomResult::Correct => "Correct",
                    RandomResult::Incorrect => "Incorrect",
                }
            })
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green))
            .border_type(BorderType::Rounded);
        let prompt_block = Block::new()
            .title_top("Continue for another game?")
            .title_bottom("(j) down / (k) up / (l) select")
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .borders(Borders::TOP | Borders::BOTTOM);

        let result_space = result_block.inner(layout[0]);
        let prompt_space = prompt_block.inner(layout[1]);

        result_block.render(layout[0], buf);
        prompt_block.render(layout[1], buf);

        let result_text = Text::styled(self.chat_completion_output.clone(), Color::Green);
        result_text.render(result_space, buf);

        let prompt_layout =
            Layout::vertical([Constraint::Max(1), Constraint::Max(1)]).split(prompt_space);
        match screen {
            EndMenuItem::Repeat => {
                let yes = Line::styled("Yes", Style::default().bg(Color::Green).fg(Color::White))
                    .centered();
                let no = Line::styled("No", Color::Green).centered();

                yes.render(prompt_layout[0], buf);
                no.render(prompt_layout[1], buf);
            }
            EndMenuItem::Exit => {
                let yes = Line::styled("Yes", Color::Green).centered();
                let no = Line::styled("No", Style::default().bg(Color::Green).fg(Color::White))
                    .centered();

                yes.render(prompt_layout[0], buf);
                no.render(prompt_layout[1], buf);
            }
        }
    }
}
