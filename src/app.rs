//! This module contains support for the business logic of the application's UI. This includes input
//! handling events and reactive changes to the persistent state of the application.

use std::time::Duration;

use clap::Parser as _;
use color_eyre::Result;
use fastrand::Rng;
use ratatui::{
    crossterm::event::{poll, read, Event, KeyCode},
    prelude::{Buffer, Rect, Widget},
    text::Line,
    widgets::Clear,
    DefaultTerminal,
};
use regex::Regex;
use ureq::agent;

use crate::utils::{
    ChatCompletionResponse, Cli, EndMenuItem, GameItem, GameScreen, MainMenuItem,
    ModelListResponse, ModelMenuDirection, OperationType, OptionsMenuItem, RandomResult, Request,
    Screen,
};

/// This structure holds information about the application itself, keeping inside it both state and
/// functions relative to the drawing and updating of the state.
pub struct App<'line> {
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

impl App<'_> {
    /// Retrieves the currently stored value for the [`screen`] field as an immutable reference.
    pub(crate) fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Retrieves the currently stored value for the [`screen`] field as a mutable reference.
    pub(crate) fn screen_mut(&mut self) -> &mut Screen {
        &mut self.screen
    }

    /// This function serves as a way of fetching the models currently available for use through the
    /// OpenRouter API. Note it does not require any type of authentication so the API key is not
    /// used.
    fn fetch_models(&mut self) {
        let response: ModelListResponse = ureq::get("https://openrouter.ai/api/v1/models")
            .call()
            .expect("models request failed")
            .into_body()
            .read_json()
            .expect("json failed to parse");

        for model in response.data() {
            self.models.push(model.id().to_string());
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
                        .choices()
                        .last()
                        .expect("empty vector when processing request")
                        .message()
                        .content()
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
                .model()
                .unwrap_or_else(|| "qwen/qwen3-32b:free".to_owned()),
            models: Vec::new(),
            models_view: Vec::new(),
            selectors_view: Vec::new(),
            model_view_selected: String::new(),
            model_view_offset: 0,
            api_key: cli.api_key(),
            ranged_re: Regex::new(r"\A\d+\.\.\d+\z").expect("bad regex syntax"),
            input_re: Regex::new(r"\A\d+\z").expect("bad regex syntax"),
            extra_line_help: false,
            processing_request: false,
            rng: Rng::new(),
            chat_completion_output: String::new(),
        }
    }
}
