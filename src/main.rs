#![expect(unused, reason = "Temporary allow during development.")]

use clap::Parser;
use color_eyre::{
    eyre::{bail, Result},
    install,
};
use ratatui::{
    crossterm::event::{read, Event, KeyCode},
    init,
    layout::Flex,
    prelude::{Alignment, Buffer, Color, Constraint, Layout, Line, Rect, Style, Widget},
    restore,
    widgets::{Block, BorderType, Clear},
    DefaultTerminal,
};
use serde::Deserialize;

fn main() -> Result<()> {
    install()?;
    let cli = Cli::parse();

    let terminal = init();
    let app_result = App::default().run(terminal);
    restore();
    app_result
}

#[derive(Parser)]
#[command(name = "randy-ng", version, about, long_about = None)]
struct Cli {
    /// The OpenRouter model to use for the AI request.
    ///
    /// This should be set through the command-line, the environment variable or the in-game menu.
    /// If not setting it through the in-game menu, one must use the name in the OpenRouter model
    /// page that appears right below the public-facing name.
    #[arg(short, long, env = "OPENROUTER_MODEL", value_name = "MODEL_NAME")]
    model: Option<String>,
}

enum Screen {
    MainMenu(MainMenuItem),
    OptionsMenu(OptionsMenuItem),
    InGame(GameScreen),
    ModelMenu,
}

enum MainMenuItem {
    Play,
    Options,
    Exit,
}

enum OptionsMenuItem {
    Model,
    Return,
}

enum GameScreen {
    Game(GameItem),
    PauseMenu(PauseMenuItem),
    EndMenu(EndMenuItem),
}

enum GameItem {
    Range,
    Input,
}

enum PauseMenuItem {
    Model,
    Exit,
}

enum EndMenuItem {
    Repeat,
    Exit,
}

#[derive(Deserialize)]
struct Response {
    data: Vec<Data>,
}

#[derive(Deserialize)]
struct Data {
    id: String,
}

struct App<'a> {
    exit: bool,
    screen: Screen,
    score: u8,
    range_input: String,
    input: String,
    model: String,
    models: Vec<String>,
    models_view: Vec<Line<'a>>,
    model_view_selected: String,
    model_view_offset: u16,
    api_key: String,
}

impl Default for App<'_> {
    fn default() -> Self {
        let cli = Cli::parse();

        Self {
            exit: false,
            screen: Screen::MainMenu(MainMenuItem::Play),
            score: 0,
            range_input: String::new(),
            input: String::new(),
            model: cli.model.unwrap_or("qwen/qwen3-32b:free".to_owned()),
            models: Vec::new(),
            models_view: Vec::new(),
            model_view_selected: String::new(),
            model_view_offset: 0,
            api_key: String::new(),
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
            Screen::InGame(_) => {}
            Screen::ModelMenu => self.model_menu(area, buf),
        };
    }
}

impl App<'_> {
    fn fetch_models(&mut self) {
        let response: Response = ureq::get("https://openrouter.ai/api/v1/models")
            .call()
            .expect("request failed")
            .into_body()
            .read_json()
            .expect("json failed to parse");

        for model in response.data {
            self.models.push(model.id);
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
        if let Event::Key(k) = read()? {
            match k.code {
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
                        for (idx, model) in self.models.iter().enumerate() {
                            if *model == self.model_view_selected
                                && model != self.models.last().unwrap()
                            {
                                self.model_view_selected =
                                    self.models.get(idx + 1).unwrap().to_owned();
                                break;
                            }
                        }

                        if self.model_view_selected == self.models_view.last().unwrap().to_string()
                            && self.model_view_selected != *self.models.last().unwrap()
                        {
                            self.model_view_offset += 1;
                        }
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
                        for (idx, model) in self.models.iter().enumerate() {
                            if *model == self.model_view_selected
                                && model != self.models.first().unwrap()
                            {
                                self.model_view_selected =
                                    self.models.get(idx - 1).unwrap().to_owned();
                                break;
                            }
                        }

                        if self.model_view_selected == self.models_view.first().unwrap().to_string()
                            && self.model_view_selected != *self.models.first().unwrap()
                        {
                            self.model_view_offset -= 1;
                        }
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
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }

    fn clear(&self, area: Rect, buf: &mut Buffer) {
        let clear = Clear;
        clear.render(area, buf);
    }

    fn init_menu(&self, area: Rect, title: &str, subtitle: &str, buf: &mut Buffer) -> Rect {
        let screen = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(area);

        let space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(screen[1])[1];

        let block = Block::bordered()
            .title_top(title)
            .title_bottom(subtitle)
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(Color::Green));

        block.clone().render(space, buf);

        block.inner(space)
    }

    fn main_menu(&self, area: Rect, buf: &mut Buffer, screen: &MainMenuItem) {
        self.clear(area, buf);

        let item_space = self.init_menu(area, "Main menu", "(j) down / (k) up / (l) return", buf);
        let layout = Layout::vertical([Constraint::Max(1); 3])
            .flex(Flex::SpaceBetween)
            .vertical_margin(2)
            .split(item_space);

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

        items[0].clone().render(layout[0], buf);
        items[1].clone().render(layout[1], buf);
        items[2].clone().render(layout[2], buf);
    }

    fn options_menu(&self, area: Rect, buf: &mut Buffer, screen: &OptionsMenuItem) {
        self.clear(area, buf);

        let item_space =
            self.init_menu(area, "Options menu", "(j) down / (k) up / (l) return", buf);
        let layout = Layout::vertical([Constraint::Min(1); 2])
            .flex(Flex::SpaceBetween)
            .vertical_margin(2)
            .split(item_space);

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

        items[0].clone().render(layout[0], buf);
        items[1].clone().render(layout[1], buf);
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
            .title_bottom(Line::raw("(j) down / (k) up").alignment(Alignment::Center))
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
        for model in self.models.iter().skip(self.model_view_offset as usize) {
            if *model == self.model_view_selected {
                self.models_view
                    .push(Line::styled(model.to_owned(), active_content_style));
            } else {
                self.models_view
                    .push(Line::styled(model.to_owned(), content_style));
            }
        }
        self.models_view.truncate(list_space.height as usize);

        let list_space_layout =
            Layout::vertical(vec![Constraint::Max(1); list_space.height as usize])
                .split(list_space);
        for (idx, model) in self.models_view.iter().enumerate() {
            model.render(list_space_layout[idx], buf);
        }
    }
}
