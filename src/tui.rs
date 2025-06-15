use crate::models::Project;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
    Frame, Terminal,
};
use std::io;

const PRIMARY_COLOR: Color = Color::Rgb(99, 102, 241);
const SECONDARY_COLOR: Color = Color::Rgb(139, 92, 246);
const SUCCESS_COLOR: Color = Color::Rgb(34, 197, 94);
const WARNING_COLOR: Color = Color::Rgb(251, 191, 36);
const ERROR_COLOR: Color = Color::Rgb(239, 68, 68);
const SURFACE_COLOR: Color = Color::Rgb(30, 41, 59);
const TEXT_PRIMARY: Color = Color::Rgb(248, 250, 252);
const TEXT_SECONDARY: Color = Color::Rgb(148, 163, 184);
const TEXT_MUTED: Color = Color::Rgb(100, 116, 139);
const ACCENT_COLOR: Color = Color::Rgb(20, 184, 166);

pub struct TuiApp {
    input: String,
    projects: Vec<Project>,
    filtered_projects: Vec<(usize, i64)>,
    selected_index: usize,
    matcher: SkimMatcherV2,
    should_quit: bool,
    selected_project: Option<Project>,

    project_exists_cache: Vec<bool>,

    github_status_cache: String,
    gitlab_status_cache: String,
}

impl TuiApp {
    pub fn new(projects: Vec<Project>) -> Self {
        let project_exists_cache: Vec<bool> = projects
            .iter()
            .map(|project| project.path.exists())
            .collect();

        let projects_clone = projects.clone();
        let github_thread =
            std::thread::spawn(move || Self::compute_github_status(&projects_clone));

        let projects_clone = projects.clone();
        let gitlab_thread =
            std::thread::spawn(move || Self::compute_gitlab_status(&projects_clone));

        let github_status_cache = github_thread.join().unwrap_or_else(|_| "error".to_string());
        let gitlab_status_cache = gitlab_thread.join().unwrap_or_else(|_| "error".to_string());

        let mut app = Self {
            input: String::new(),
            filtered_projects: Vec::new(),
            selected_index: 0,
            matcher: SkimMatcherV2::default(),
            should_quit: false,
            selected_project: None,
            projects,
            project_exists_cache,
            github_status_cache,
            gitlab_status_cache,
        };
        app.update_filtered_projects();
        app
    }

    pub fn run_interactive<B: Backend>(
        projects: Vec<Project>,
        terminal: &mut Terminal<B>,
    ) -> Result<Option<Project>> {
        let mut app = TuiApp::new(projects);

        loop {
            terminal.draw(|f| app.draw(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                        }
                        KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Enter => {
                            if let Some(project) = app.get_selected_project() {
                                app.selected_project = Some(project);
                                app.should_quit = true;
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                            app.update_filtered_projects();
                            app.selected_index = 0;
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                            app.update_filtered_projects();
                            app.selected_index = 0;
                        }
                        KeyCode::Up => {
                            app.move_selection_up();
                        }
                        KeyCode::Down => {
                            app.move_selection_down();
                        }
                        _ => {}
                    }
                }
            }

            if app.should_quit {
                break;
            }
        }

        Ok(app.selected_project)
    }

    fn update_filtered_projects(&mut self) {
        if self.input.is_empty() {
            self.filtered_projects = self
                .projects
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 100))
                .take(20)
                .collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .projects
                .iter()
                .enumerate()
                .filter_map(|(i, project)| {
                    self.matcher
                        .fuzzy_match(&project.name, &self.input)
                        .map(|score| (i, score))
                })
                .collect();

            scored.sort_by(|a, b| b.1.cmp(&a.1));

            self.filtered_projects = scored.into_iter().take(20).collect();
        }

        if self.selected_index >= self.filtered_projects.len() {
            self.selected_index = 0;
        }
    }

    fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_selection_down(&mut self) {
        if self.selected_index + 1 < self.filtered_projects.len() {
            self.selected_index += 1;
        }
    }

    fn get_selected_project(&self) -> Option<Project> {
        self.filtered_projects
            .get(self.selected_index)
            .and_then(|(index, _)| self.projects.get(*index))
            .cloned()
    }

    fn get_github_status(&self) -> &str {
        &self.github_status_cache
    }

    fn get_gitlab_status(&self) -> &str {
        &self.gitlab_status_cache
    }

    fn compute_github_status(projects: &[Project]) -> String {
        let has_github_projects = projects
            .iter()
            .any(|p| p.source == crate::models::ProjectSource::GitHub);

        if !has_github_projects {
            return "not configured".to_string();
        }

        if !crate::scanner::github::is_gh_installed() {
            return "CLI not found".to_string();
        }

        match crate::scanner::github::is_gh_authenticated() {
            Ok(true) => "✅ authenticated".to_string(),
            Ok(false) => "❌ not authenticated".to_string(),
            Err(_) => "❌ error checking auth".to_string(),
        }
    }

    fn compute_gitlab_status(projects: &[Project]) -> String {
        let has_gitlab_projects = projects
            .iter()
            .any(|p| p.source == crate::models::ProjectSource::GitLab);

        if !has_gitlab_projects {
            return "not configured".to_string();
        }

        if !crate::scanner::gitlab::is_glab_installed() {
            return "CLI not found".to_string();
        }

        if crate::scanner::gitlab::is_glab_accessible() {
            "✅ accessible".to_string()
        } else {
            "❌ not accessible".to_string()
        }
    }

    fn draw(&self, f: &mut Frame) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(4),
            ])
            .split(f.area());

        let title = Paragraph::new(" Project Switcher")
            .style(
                Style::default()
                    .fg(PRIMARY_COLOR)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        f.render_widget(title, main_chunks[0]);

        let search_placeholder = if self.input.is_empty() {
            "Type to search projects..."
        } else {
            ""
        };

        let search_content = if self.input.is_empty() {
            Text::from(vec![
                Line::from(vec![Span::styled("", Style::default())]),
                Line::from(vec![Span::styled(
                    search_placeholder,
                    Style::default().fg(TEXT_MUTED).italic(),
                )]),
            ])
        } else {
            Text::from(vec![
                Line::from(vec![
                    Span::styled("🔍 ", Style::default().fg(ACCENT_COLOR)),
                    Span::styled(&self.input, Style::default().fg(TEXT_PRIMARY)),
                    Span::styled("│", Style::default().fg(PRIMARY_COLOR).slow_blink()),
                ]),
                Line::from(vec![Span::styled("", Style::default())]),
            ])
        };

        let search_box = Paragraph::new(search_content).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if self.input.is_empty() {
                    TEXT_MUTED
                } else {
                    PRIMARY_COLOR
                }))
                .title(Line::from(vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        "Search",
                        Style::default()
                            .fg(TEXT_PRIMARY)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default()),
                ]))
                .title_style(Style::default().fg(TEXT_PRIMARY))
                .padding(Padding::horizontal(2)),
        );
        f.render_widget(search_box, main_chunks[1]);

        let items: Vec<ListItem> = self
            .filtered_projects
            .iter()
            .enumerate()
            .map(|(i, (project_index, _score))| {
                let project = &self.projects[*project_index];
                let is_selected = i == self.selected_index;

                let (source_icon, source_color, source_label) = match project.source {
                    crate::models::ProjectSource::Local => ("📂", SUCCESS_COLOR, "Local"),
                    crate::models::ProjectSource::Cursor => ("🎯", PRIMARY_COLOR, "Cursor"),
                    crate::models::ProjectSource::GitHub => ("🐙", SECONDARY_COLOR, "GitHub"),
                    crate::models::ProjectSource::GitLab => ("🦊", ACCENT_COLOR, "GitLab"),
                };

                let status_indicator = if project.source == crate::models::ProjectSource::GitHub
                    || project.source == crate::models::ProjectSource::GitLab
                {
                    if self.project_exists_cache[*project_index] {
                        ("✓", SUCCESS_COLOR, "Cloned")
                    } else {
                        ("⚡", WARNING_COLOR, "Remote")
                    }
                } else {
                    ("●", SUCCESS_COLOR, "Available")
                };

                let time_str = if let Some(timestamp) = project.last_modified {
                    format!(" • {}", timestamp.format("%m/%d %H:%M"))
                } else {
                    String::new()
                };

                let mut line_spans = vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(source_icon, Style::default().fg(source_color)),
                    Span::styled("  ", Style::default()),
                ];

                if is_selected {
                    line_spans.extend(vec![
                        Span::styled(
                            "▶ ",
                            Style::default()
                                .fg(ACCENT_COLOR)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            &project.name,
                            Style::default()
                                .fg(TEXT_PRIMARY)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]);
                } else {
                    line_spans.extend(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(&project.name, Style::default().fg(TEXT_PRIMARY)),
                    ]);
                }

                line_spans.extend(vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(status_indicator.0, Style::default().fg(status_indicator.1)),
                    Span::styled(time_str, Style::default().fg(TEXT_SECONDARY)),
                ]);

                if is_selected {
                    line_spans.extend(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled(
                            format!("[{}]", source_label),
                            Style::default()
                                .fg(source_color)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]);
                }

                let item_style = if is_selected {
                    Style::default().bg(SURFACE_COLOR).fg(TEXT_PRIMARY)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(line_spans)).style(item_style)
            })
            .collect();

        let projects_title = format!(
            " Projects ({}/{}) ",
            self.filtered_projects.len(),
            self.projects.len()
        );

        let projects_list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(TEXT_MUTED))
                .title(Line::from(vec![Span::styled(
                    projects_title,
                    Style::default()
                        .fg(TEXT_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                )]))
                .padding(Padding::horizontal(1)),
        );

        f.render_widget(projects_list, main_chunks[3]);

        let github_status = self.get_github_status();
        let github_status_color = if github_status.contains("✅") {
            SUCCESS_COLOR
        } else if github_status.contains("❌") {
            ERROR_COLOR
        } else {
            WARNING_COLOR
        };

        let gitlab_status = self.get_gitlab_status();
        let gitlab_status_color = if gitlab_status.contains("✅") {
            SUCCESS_COLOR
        } else if gitlab_status.contains("❌") {
            ERROR_COLOR
        } else {
            WARNING_COLOR
        };

        let status_content = Text::from(vec![Line::from(vec![
            Span::styled("🐙 GitHub: ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(github_status, Style::default().fg(github_status_color)),
            Span::styled("  │  ", Style::default().fg(TEXT_MUTED)),
            Span::styled("🦊 GitLab: ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(gitlab_status, Style::default().fg(gitlab_status_color)),
            Span::styled("  │  ", Style::default().fg(TEXT_MUTED)),
            Span::styled("📊 Total: ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                format!("{}", self.projects.len()),
                Style::default()
                    .fg(ACCENT_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(TEXT_MUTED)),
            Span::styled("🔍 Shown: ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                format!("{}", self.filtered_projects.len()),
                Style::default()
                    .fg(PRIMARY_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
        ])]);

        let status_bar = Paragraph::new(status_content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(TEXT_MUTED))
                    .title(" Status ")
                    .title_style(Style::default().fg(TEXT_SECONDARY))
                    .padding(Padding::horizontal(2)),
            )
            .alignment(Alignment::Center);
        f.render_widget(status_bar, main_chunks[5]);

        let help_content = Text::from(vec![Line::from(vec![
            Span::styled(
                "↑↓",
                Style::default()
                    .fg(ACCENT_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Navigate  ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(SUCCESS_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Select  ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                "Esc/q",
                Style::default()
                    .fg(ERROR_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Quit", Style::default().fg(TEXT_SECONDARY)),
        ])]);

        let help_box = Paragraph::new(help_content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(TEXT_MUTED))
                    .title(" Help ")
                    .title_style(Style::default().fg(TEXT_SECONDARY))
                    .padding(Padding::horizontal(2)),
            )
            .alignment(Alignment::Center);
        f.render_widget(help_box, main_chunks[6]);
    }
}

pub fn run_interactive_mode(projects: Vec<Project>) -> Result<Option<Project>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = TuiApp::run_interactive(projects, &mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Project, ProjectSource};
    use chrono::Utc;
    use std::path::PathBuf;

    fn create_test_projects() -> Vec<Project> {
        vec![
            Project {
                name: "awesome-project".to_string(),
                path: PathBuf::from("/path/to/awesome-project"),
                source: ProjectSource::Local,
                last_modified: Some(Utc::now()),
                github_url: None,
                gitlab_url: None,
            },
            Project {
                name: "cool-app".to_string(),
                path: PathBuf::from("/path/to/cool-app"),
                source: ProjectSource::Local,
                last_modified: Some(Utc::now()),
                github_url: None,
                gitlab_url: None,
            },
            Project {
                name: "my-website".to_string(),
                path: PathBuf::from("/path/to/my-website"),
                source: ProjectSource::Local,
                last_modified: Some(Utc::now()),
                github_url: None,
                gitlab_url: None,
            },
            Project {
                name: "switchr".to_string(),
                path: PathBuf::from("/path/to/switchr"),
                source: ProjectSource::Local,
                last_modified: Some(Utc::now()),
                github_url: None,
                gitlab_url: None,
            },
        ]
    }

    #[test]
    fn test_new_tui_app() {
        let projects = create_test_projects();
        let app = TuiApp::new(projects.clone());

        assert_eq!(app.input, "");
        assert_eq!(app.projects.len(), 4);
        assert_eq!(app.selected_index, 0);
        assert!(!app.should_quit);
        assert!(app.selected_project.is_none());
    }

    #[test]
    fn test_initial_filtered_projects() {
        let projects = create_test_projects();
        let app = TuiApp::new(projects);

        assert_eq!(app.filtered_projects.len(), 4);
        assert_eq!(app.filtered_projects[0].0, 0);
    }

    #[test]
    fn test_fuzzy_search_exact_match() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.input = "switchr".to_string();
        app.update_filtered_projects();

        assert_eq!(app.filtered_projects.len(), 1);
        assert_eq!(app.filtered_projects[0].0, 3);
    }

    #[test]
    fn test_fuzzy_search_partial_match() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.input = "proj".to_string();
        app.update_filtered_projects();

        assert_eq!(app.filtered_projects.len(), 1);
        assert_eq!(app.filtered_projects[0].0, 0);
    }

    #[test]
    fn test_fuzzy_search_multiple_matches() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.input = "app".to_string();
        app.update_filtered_projects();

        assert!(!app.filtered_projects.is_empty());

        assert!(app.filtered_projects.iter().any(|(i, _)| *i == 1));
    }

    #[test]
    fn test_fuzzy_search_no_matches() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.input = "nonexistent".to_string();
        app.update_filtered_projects();

        assert_eq!(app.filtered_projects.len(), 0);
    }

    #[test]
    fn test_selection_navigation() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        assert_eq!(app.selected_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_index, 1);

        app.move_selection_down();
        assert_eq!(app.selected_index, 2);

        app.move_selection_up();
        assert_eq!(app.selected_index, 1);

        app.move_selection_up();
        assert_eq!(app.selected_index, 0);

        app.move_selection_up();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_selection_bounds_with_filtered_results() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.input = "switchr".to_string();
        app.update_filtered_projects();

        assert_eq!(app.filtered_projects.len(), 1);
        assert_eq!(app.selected_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_get_selected_project() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects.clone());

        let selected = app.get_selected_project().unwrap();
        assert_eq!(selected.name, "awesome-project");

        app.move_selection_down();
        let selected = app.get_selected_project().unwrap();
        assert_eq!(selected.name, "cool-app");
    }

    #[test]
    fn test_get_selected_project_with_search() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.input = "switchr".to_string();
        app.update_filtered_projects();

        let selected = app.get_selected_project().unwrap();
        assert_eq!(selected.name, "switchr");
    }

    #[test]
    fn test_selection_reset_on_search() {
        let projects = create_test_projects();
        let mut app = TuiApp::new(projects);

        app.move_selection_down();
        app.move_selection_down();
        assert_eq!(app.selected_index, 2);

        app.input = "app".to_string();
        app.update_filtered_projects();

        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_shows_top_20_matches_only() {
        let mut projects = Vec::new();
        for i in 0..25 {
            projects.push(Project {
                name: format!("project-{:02}", i),
                path: PathBuf::from(format!("/path/to/project-{:02}", i)),
                source: ProjectSource::Local,
                last_modified: Some(Utc::now()),
                github_url: None,
                gitlab_url: None,
            });
        }

        let app = TuiApp::new(projects);

        assert_eq!(app.filtered_projects.len(), 20);
    }
}
