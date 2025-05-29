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
                Error::StatusCode(s) => match s {
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

#[derive(PartialEq)]
enum Screen {
    MainMenu(MainMenuItem),
    OptionsMenu(OptionsMenuItem),
    InGame(GameScreen),
    ModelMenu,
}

#[derive(PartialEq)]
enum MainMenuItem {
    Play,
    Options,
    Exit,
}

#[derive(PartialEq)]
enum OptionsMenuItem {
    Model,
    Return,
}

#[derive(PartialEq)]
enum GameScreen {
    Game(GameItem),
    EndMenu(EndMenuItem),
}

#[derive(PartialEq)]
enum GameItem {
    Range,
    Input,
}

#[derive(PartialEq)]
enum EndMenuItem {
    Repeat,
    Exit,
}

#[derive(Clone, Copy)]
enum RandomResult {
    Correct,
    Incorrect,
}

static LLM_INPUT: LazyLock<&str> = LazyLock::new(|| {
    "You will answer only to \"Correct\" or \"Incorrect.\" These correspond to either a\
notification that a user got a number right in a number guessing game or not, respectively. Your\
task is to, depending on whether you were notified they got it right, or not, to return a\
cowboy-like answer to the user. Make it a short text. Include just your answer and nothing more.\
Don't include emoji or otherwise non-verbal content."
});

#[derive(Serialize)]
struct Request {
    model: String,
    messages: Vec<Message>,
}

impl Request {
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

#[derive(Serialize, Deserialize)]
struct Message {
    role: Role,
    content: String,
}

impl Message {
    fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    System,
    Assistant,
    User,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choices>,
}

#[derive(Deserialize)]
struct Choices {
    message: Message,
}

#[derive(Deserialize)]
struct Response {
    data: Vec<Data>,
}

#[derive(Deserialize)]
struct Data {
    id: String,
}

enum MenuType {
    MainMenu(u8),
    OptionsMenu(u8),
}

impl MenuType {
    fn repr(&self) -> &str {
        match self {
            MenuType::MainMenu(_) => "Main menu",
            MenuType::OptionsMenu(_) => "Options menu",
        }
    }
}

struct App<'a> {
    exit: bool,
    screen: Screen,
    score: u8,
    range_input: String,
    input: String,
    result: Option<RandomResult>,
    model: String,
    models: Vec<String>,
    models_view: Vec<Line<'a>>,
    selectors_view: Vec<Line<'a>>,
    model_view_selected: String,
    model_view_offset: u16,
    api_key: String,
    ranged_re: Regex,
    input_re: Regex,
    extra_line_help: bool,
    processing_request: bool,
    rng: Rng,
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
            model: cli.model.unwrap_or("qwen/qwen3-32b:free".to_owned()),
            models: Vec::new(),
            models_view: Vec::new(),
            selectors_view: Vec::new(),
            model_view_selected: String::new(),
            model_view_offset: 0,
            api_key: cli.api_key,
            ranged_re: Regex::new(r"\A\d+\.\.\d+\z").unwrap(),
            input_re: Regex::new(r"\A\d+\z").unwrap(),
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
            Screen::MainMenu(s) => {
                self.main_menu(area, buf, s);
            }
            Screen::OptionsMenu(s) => {
                self.options_menu(area, buf, s);
            }
            Screen::InGame(s) => match s {
                GameScreen::Game(s) => self.take_input(area, buf, s),
                GameScreen::EndMenu(s) => self.end_menu(area, buf, s),
            },
            Screen::ModelMenu => self.model_menu(area, buf),
        };
    }
}

impl App<'_> {
    fn fetch_models(&mut self) {
        let response: Response = ureq::get("https://openrouter.ai/api/v1/models")
            .call()
            .expect("models request failed")
            .into_body()
            .read_json()
            .expect("json failed to parse");

        for model in response.data {
            self.models.push(model.id);
        }
    }

    fn validate_input(&self) -> bool {
        if self.ranged_re.is_match(&self.range_input) && self.input_re.is_match(&self.input) {
            // process the ranged input
            let (start, end) = self
                .range_input
                .split_at(self.range_input.find("..").unwrap());
            let end: String = end.chars().rev().collect();
            let (end, _) = end.split_at(end.find("..").unwrap());
            let start: usize = start.parse().unwrap();
            let end: usize = end.parse().unwrap();
            let mut flag1 = false;

            if start < end {
                flag1 = true;
            }

            // process the guess input
            let guess: usize = self.input.parse().unwrap();
            let mut flag2 = false;

            if guess >= start && guess <= end {
                flag2 = true;
            }

            return flag1 && flag2;
        }

        false
    }

    fn process_random(&mut self) {
        let (start, end) = self
            .range_input
            .split_at(self.range_input.find("..").unwrap());
        let end: String = end.chars().rev().collect();
        let (end, _) = end.split_at(end.find("..").unwrap());

        let start: usize = start.parse().unwrap();
        let end: usize = end.parse().unwrap();
        let guess: usize = self.input.parse().unwrap();

        let random = self.rng.usize(start..=end);

        if guess == random {
            self.result = Some(RandomResult::Correct);
        } else {
            self.result = Some(RandomResult::Incorrect);
        }
    }

    fn process_request(&mut self) -> Result<()> {
        let request_body = Request::new(self.model.clone(), self.result.unwrap());
        let agent = agent();

        loop {
            match agent
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send_json(&request_body)
            {
                Ok(response) => {
                    let response: ChatCompletionResponse = response.into_body().read_json()?;
                    let output = response.choices.last().unwrap().message.content.clone();

                    if output.is_empty() {
                        continue;
                    } else {
                        self.chat_completion_output = output;
                        break Ok(());
                    }
                }
                Err(err) => break Err(err.into()),
            }
        }
    }

    fn run(&mut self, mut term: DefaultTerminal) -> Result<()> {
        while !self.exit {
            term.draw(|f| f.render_widget(&mut *self, f.area()))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if self.processing_request {
            self.process_random();
            self.process_request()?;
            self.screen = Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat));
            self.processing_request = false;
        }

        if poll(Duration::from_millis(100)).is_ok_and(|v| v) {
            if let Event::Key(k) = read()? {
                match k.code {
                    KeyCode::Char(c)
                        if self.screen == Screen::InGame(GameScreen::Game(GameItem::Range))
                            && !self.processing_request =>
                    {
                        self.range_input.push(c);
                    }
                    KeyCode::Char(c)
                        if self.screen == Screen::InGame(GameScreen::Game(GameItem::Input))
                            && !self.processing_request =>
                    {
                        self.input.push(c);
                    }
                    KeyCode::Tab
                        if self.screen == Screen::InGame(GameScreen::Game(GameItem::Range))
                            && !self.processing_request =>
                    {
                        self.screen = Screen::InGame(GameScreen::Game(GameItem::Input));
                    }
                    KeyCode::Tab
                        if self.screen == Screen::InGame(GameScreen::Game(GameItem::Input))
                            && !self.processing_request =>
                    {
                        self.screen = Screen::InGame(GameScreen::Game(GameItem::Range));
                    }
                    KeyCode::Backspace
                        if self.screen == Screen::InGame(GameScreen::Game(GameItem::Range))
                            && !self.processing_request =>
                    {
                        self.range_input.pop();
                    }
                    KeyCode::Backspace
                        if self.screen == Screen::InGame(GameScreen::Game(GameItem::Input))
                            && !self.processing_request =>
                    {
                        self.input.pop();
                    }
                    KeyCode::Enter
                        if (self.screen == Screen::InGame(GameScreen::Game(GameItem::Range))
                            || self.screen
                                == Screen::InGame(GameScreen::Game(GameItem::Input)))
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
                    KeyCode::Char('j') => match &self.screen {
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
                            let mut first_model_after_view = String::new();

                            for (idx, model) in self.models.iter().enumerate() {
                                if *model == self.model_view_selected
                                    && model != self.models.last().unwrap()
                                {
                                    if self.model_view_selected
                                        == self.models_view.last().unwrap().to_string()
                                    {
                                        first_model_after_view =
                                            self.models.get(idx + 1).unwrap().clone();
                                    }

                                    self.model_view_selected =
                                        self.models.get(idx + 1).unwrap().to_owned();
                                    break;
                                }
                            }

                            if !first_model_after_view.is_empty()
                                && self.model_view_selected == first_model_after_view
                            {
                                self.model_view_offset += 1;
                            }
                        }
                        Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat)) => {
                            self.screen = Screen::InGame(GameScreen::EndMenu(EndMenuItem::Exit));
                        }
                        _ => {}
                    },
                    KeyCode::Char('k') => match &self.screen {
                        Screen::MainMenu(MainMenuItem::Exit) => {
                            self.screen = Screen::MainMenu(MainMenuItem::Options);
                        }
                        Screen::MainMenu(MainMenuItem::Options) => {
                            self.screen = Screen::MainMenu(MainMenuItem::Play);
                        }
                        Screen::OptionsMenu(OptionsMenuItem::Return) => {
                            self.screen = Screen::OptionsMenu(OptionsMenuItem::Model)
                        }
                        Screen::ModelMenu => {
                            let mut first_model_before_view = String::new();

                            for (idx, model) in self.models.iter().enumerate() {
                                if *model == self.model_view_selected
                                    && model != self.models.first().unwrap()
                                {
                                    if self.model_view_selected
                                        == self.models_view.first().unwrap().to_string()
                                    {
                                        first_model_before_view =
                                            self.models.get(idx - 1).unwrap().clone();
                                    }

                                    self.model_view_selected =
                                        self.models.get(idx - 1).unwrap().to_owned();
                                    break;
                                }
                            }

                            if !first_model_before_view.is_empty()
                                && self.model_view_selected == first_model_before_view
                            {
                                self.model_view_offset -= 1;
                            }
                        }
                        Screen::InGame(GameScreen::EndMenu(EndMenuItem::Exit)) => {
                            self.screen = Screen::InGame(GameScreen::EndMenu(EndMenuItem::Repeat));
                        }
                        _ => {}
                    },
                    KeyCode::Char('l') => match &self.screen {
                        Screen::MainMenu(MainMenuItem::Play) => {
                            self.screen = Screen::InGame(GameScreen::Game(GameItem::Range))
                        }
                        Screen::MainMenu(MainMenuItem::Options) => {
                            self.screen = Screen::OptionsMenu(OptionsMenuItem::Model)
                        }
                        Screen::MainMenu(MainMenuItem::Exit) => self.exit = true,
                        Screen::OptionsMenu(OptionsMenuItem::Model) => {
                            self.screen = Screen::ModelMenu;

                            self.model_view_offset = 0;
                            self.fetch_models();
                            self.model_view_selected = self.models.first().unwrap().to_owned();
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
                    },
                    KeyCode::Char('h') => {
                        if let Screen::ModelMenu = &self.screen {
                            self.screen = Screen::OptionsMenu(OptionsMenuItem::Model);
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn clear(&self, area: Rect, buf: &mut Buffer) {
        let clear = Clear;
        clear.render(area, buf);
    }

    fn init_menu(&self, area: Rect, buf: &mut Buffer, menu: MenuType) -> Rc<[Rect]> {
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

    fn main_menu(&self, area: Rect, buf: &mut Buffer, screen: &MainMenuItem) {
        self.clear(area, buf);

        let item_layout = self.init_menu(area, buf, MenuType::MainMenu(3));

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

    fn options_menu(&self, area: Rect, buf: &mut Buffer, screen: &OptionsMenuItem) {
        self.clear(area, buf);

        let item_layout = self.init_menu(area, buf, MenuType::OptionsMenu(2));

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

    fn model_menu(&mut self, area: Rect, buf: &mut Buffer) {
        self.clear(area, buf);

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

    fn take_input(&self, area: Rect, buf: &mut Buffer, screen: &GameItem) {
        self.clear(area, buf);

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

    fn end_menu(&self, area: Rect, buf: &mut Buffer, screen: &EndMenuItem) {
        self.clear(area, buf);

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
                match self.result.unwrap() {
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
