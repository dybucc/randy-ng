//! This module contains support for UI rendering. It includes each of the main screenful state
//! renderings that compute and display on-screen the corresponding layout.

use std::rc::Rc;

use ratatui::{
    layout::Flex,
    prelude::{Alignment, Buffer, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::{bar::FULL, DOT},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};

use crate::{
    utils::{
        EndMenuItem, GameItem, GameScreen, MainMenuItem, MenuType, OptionsMenuItem, RandomResult,
        Screen,
    },
    App,
};

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
            .style(Color::Green)
            .border_type(BorderType::Rounded);
        let list_space = model_list_block.inner(space);
        let list_space =
            Layout::horizontal([Constraint::Percentage(5), Constraint::Percentage(95)])
                .split(list_space);
        // I would like to destructure the `list_space` slice with a pattern but that doesn't seem
        // possible without using a `let ... else` statement, and this function must not return
        // anything nor should it have an early return because drawing on-screen must not be
        // fallible. One way to fix it would be to change the function that actually draws on-screen
        // and the contents of the closure it gets passed so that a different function from the
        // default ratatui `render_widget` is run instead with a `Result<>` return type that
        // cascades through whatever callbacks it performs. Raincheck.
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
        .split(area);
        let main_space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(space[1])[1];
        let score_space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .flex(Flex::End)
        .split(space[2]);
        let score_space = Layout::vertical([Constraint::Max(1)])
            .flex(Flex::End)
            .split(score_space[1])[0];

        let layout = if self.extra_line_help || self.processing_request {
            Layout::vertical([Constraint::Max(3), Constraint::Max(3), Constraint::Max(1)])
                .flex(Flex::Center)
                .split(main_space)
        } else {
            Layout::vertical([Constraint::Max(3), Constraint::Max(3)])
                .flex(Flex::Center)
                .split(main_space)
        };

        let score_block = Block::new()
            .title_top(format!("Score: {}", self.score))
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .borders(Borders::TOP);

        score_block.render(score_space, buf);

        let ranged_input_block = Block::bordered()
            .title_top("Input a range in the format n..m where n < m")
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .border_type(BorderType::Rounded);
        let guess_input_block = Block::bordered()
            .title_top("Input a number in the above range")
            .title_bottom("(tab) switch between panels / (ret) continue")
            .title_alignment(Alignment::Center)
            .style(Color::Green)
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
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::TOP);

            processing_text.render(layout[2], buf);
        }

        let ranged_input_space = ranged_input_block.inner(layout[0]);
        let guess_input_space = guess_input_block.inner(layout[1]);

        ranged_input_block.render(layout[0], buf);
        guess_input_block.render(layout[1], buf);

        let mut ranged_input =
            Line::styled(self.range_input.clone(), Color::White).alignment(Alignment::Center);
        let mut input = Line::styled(self.input.clone(), Color::White).alignment(Alignment::Center);
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
        .split(area);
        let main_space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(space[1])[1];
        let score_space = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(100),
            Constraint::Percentage(40),
        ])
        .split(space[2]);
        let score_space = Layout::vertical([Constraint::Max(1)])
            .flex(Flex::End)
            .split(score_space[1])[0];

        let layout = Layout::vertical([Constraint::Min(1), Constraint::Max(4)])
            .flex(Flex::Center)
            .split(main_space);

        let score_block = Block::new()
            .title_top(format!("Score: {}", self.score))
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .borders(Borders::TOP);

        score_block.render(score_space, buf);

        let result_block = Block::bordered()
            .title_top({
                match self.result.expect("result not yet computed") {
                    RandomResult::Correct => "Correct",
                    RandomResult::Incorrect => "Incorrect",
                }
            })
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .border_type(BorderType::Rounded);
        let prompt_block = Block::new()
            .title_top("Continue for another game?")
            .title_bottom("(j) down / (k) up / (l) select")
            .title_alignment(Alignment::Center)
            .style(Color::Green)
            .borders(Borders::TOP | Borders::BOTTOM);

        let prompt_space = prompt_block.inner(layout[1]);

        prompt_block.render(layout[1], buf);

        let result_text = Paragraph::new(self.chat_completion_output.clone())
            .style(Color::Green)
            .block(result_block)
            .wrap(Wrap { trim: true });
        result_text.render(layout[0], buf);

        let content_style = Style::default().fg(Color::Green);
        let active_content_style = Style::default().fg(Color::White).bg(Color::Green);

        let prompt_layout =
            Layout::vertical([Constraint::Max(1), Constraint::Max(1)]).split(prompt_space);

        let yes;
        let no;
        match screen {
            EndMenuItem::Repeat => {
                yes = Line::styled("Yes", active_content_style).centered();
                no = Line::styled("No", content_style).centered();
            }
            EndMenuItem::Exit => {
                yes = Line::styled("Yes", content_style).centered();
                no = Line::styled("No", active_content_style).centered();
            }
        }

        yes.render(prompt_layout[0], buf);
        no.render(prompt_layout[1], buf);
    }
}
