//! Interactive TUI goal browser for selecting goals with dual-panel layout.
//!
//! This module provides a rich terminal user interface for browsing and selecting
//! goals from local and global sources, with preview capabilities.

use anyhow::{Context as AnyhowContext, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::config::DiscoveredGoal;

/// Represents which panel is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Local,
    Global,
}

/// Represents the current mode of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppMode {
    /// Browsing and selecting goals from panels
    Selection,
    /// Viewing the full content of a goal's prompt.yaml
    ViewMode,
}

/// Control flow result from input handling.
enum ControlFlow {
    /// Continue running the event loop
    Continue,
    /// User selected a goal, exit and return the goal name
    Select,
    /// User wants to quit the application
    Quit,
}

/// Main application state for the goal browser.
struct GoalBrowserApp {
    /// Local goals discovered
    local_goals: Vec<DiscoveredGoal>,
    /// Global goals discovered
    global_goals: Vec<DiscoveredGoal>,
    /// Which panel is currently active
    active_panel: Panel,
    /// Selected index in local panel
    local_selected: usize,
    /// Selected index in global panel
    global_selected: usize,
    /// Current application mode
    mode: AppMode,
    /// Scroll offset in view mode (line number)
    view_scroll: usize,
    /// Cached content of the prompt.yaml being viewed
    view_content: Option<String>,
    /// Cached path being viewed (for display)
    view_path: Option<String>,
}

impl GoalBrowserApp {
    /// Creates a new GoalBrowserApp from a list of discovered goals.
    fn new(goals: Vec<DiscoveredGoal>) -> Self {
        let mut local_goals = Vec::new();
        let mut global_goals = Vec::new();

        for goal in goals {
            match goal.source {
                crate::config::GoalSource::Local => local_goals.push(goal),
                crate::config::GoalSource::Global => global_goals.push(goal),
            }
        }

        // Determine initial active panel based on which has goals
        let active_panel = if !local_goals.is_empty() {
            Panel::Local
        } else {
            Panel::Global
        };

        Self {
            local_goals,
            global_goals,
            active_panel,
            local_selected: 0,
            global_selected: 0,
            mode: AppMode::Selection,
            view_scroll: 0,
            view_content: None,
            view_path: None,
        }
    }

    /// Returns the currently selected goal, if any.
    fn get_selected_goal(&self) -> Option<&DiscoveredGoal> {
        match self.active_panel {
            Panel::Local => self.local_goals.get(self.local_selected),
            Panel::Global => self.global_goals.get(self.global_selected),
        }
    }

    /// Returns the name of the currently selected goal.
    fn get_selected_goal_name(&self) -> Option<String> {
        self.get_selected_goal().map(|g| g.name.clone())
    }

    /// Moves selection up in the current panel.
    fn move_up(&mut self) {
        match self.active_panel {
            Panel::Local => {
                if !self.local_goals.is_empty() && self.local_selected > 0 {
                    self.local_selected -= 1;
                }
            }
            Panel::Global => {
                if !self.global_goals.is_empty() && self.global_selected > 0 {
                    self.global_selected -= 1;
                }
            }
        }
    }

    /// Moves selection down in the current panel.
    fn move_down(&mut self) {
        match self.active_panel {
            Panel::Local => {
                if self.local_selected + 1 < self.local_goals.len() {
                    self.local_selected += 1;
                }
            }
            Panel::Global => {
                if self.global_selected + 1 < self.global_goals.len() {
                    self.global_selected += 1;
                }
            }
        }
    }

    /// Toggles between local and global panels.
    fn toggle_panel(&mut self) {
        // Only toggle if both panels have goals
        let can_switch = !self.local_goals.is_empty() && !self.global_goals.is_empty();

        if can_switch {
            self.active_panel = match self.active_panel {
                Panel::Local => Panel::Global,
                Panel::Global => Panel::Local,
            };
        }
    }

    /// Enters view mode for the currently selected goal.
    fn enter_view_mode(&mut self) -> Result<()> {
        if let Some(goal) = self.get_selected_goal() {
            // Load the prompt.yaml content
            let loaded = crate::config::find_and_load_goal(&goal.name)?;

            // Read the actual prompt.yaml file
            let prompt_path = loaded.directory.join("prompt.yaml");
            let content = std::fs::read_to_string(&prompt_path)
                .with_context(|| format!("Failed to read {}", prompt_path.display()))?;

            self.view_content = Some(content);
            self.view_path = Some(prompt_path.display().to_string());
            self.view_scroll = 0;
            self.mode = AppMode::ViewMode;
        }
        Ok(())
    }

    /// Scrolls up in view mode.
    fn scroll_up(&mut self) {
        if self.view_scroll > 0 {
            self.view_scroll -= 1;
        }
    }

    /// Scrolls down in view mode.
    fn scroll_down(&mut self) {
        // We'll check bounds during rendering based on content length
        self.view_scroll += 1;
    }

    /// Scrolls up by a page in view mode.
    fn page_up(&mut self, page_size: usize) {
        self.view_scroll = self.view_scroll.saturating_sub(page_size);
    }

    /// Scrolls down by a page in view mode.
    fn page_down(&mut self, page_size: usize) {
        self.view_scroll = self.view_scroll.saturating_add(page_size);
    }
}

/// Entry point for the goal browser TUI.
///
/// Takes a list of discovered goals and returns the name of the selected goal.
pub fn run_goal_browser(goals: Vec<DiscoveredGoal>) -> Result<String> {
    // Set up terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Initialize app state
    let mut app = GoalBrowserApp::new(goals);

    // Run main event loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    // Return result
    match result {
        Ok(goal_name) => Ok(goal_name),
        Err(e) => Err(e),
    }
}

/// Main application event loop.
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut GoalBrowserApp,
) -> Result<String> {
    loop {
        terminal.draw(|f| render_ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            // Only process key press events, not release
            if key.kind == KeyEventKind::Press {
                match handle_input(key, app)? {
                    ControlFlow::Continue => {}
                    ControlFlow::Select => {
                        return app
                            .get_selected_goal_name()
                            .ok_or_else(|| anyhow::anyhow!("No goal selected"));
                    }
                    ControlFlow::Quit => {
                        anyhow::bail!("User quit goal browser");
                    }
                }
            }
        }
    }
}

/// Main UI rendering function.
fn render_ui(frame: &mut Frame, app: &GoalBrowserApp) {
    match app.mode {
        AppMode::Selection => render_selection_mode(frame, app),
        AppMode::ViewMode => render_view_mode(frame, app),
    }
}

/// Renders the ASCII art logo.
fn render_logo(area: Rect, frame: &mut Frame) {
    let bright_orange = Color::Rgb(255, 165, 0);
    let soft_neon_green = Color::Rgb(144, 238, 144);

    let logo_text = r#"________/\\\\\\\\\__/\\\_________________/\\\\\\\\\_____/\\\______________/\\\_
 _____/\\\////////__\/\\\_______________/\\\\\\\\\\\\\__\/\\\_____________\/\\\_
  ___/\\\/___________\/\\\______________/\\\/////////\\\_\/\\\_____________\/\\\_
   __/\\\_____________\/\\\_____________\/\\\_______\/\\\_\//\\\____/\\\____/\\\__
    _\/\\\_____________\/\\\_____________\/\\\\\\\\\\\\\\\__\//\\\__/\\\\\__/\\\___
     _\//\\\____________\/\\\_____________\/\\\/////////\\\___\//\\\/\\\/\\\/\\\____
      __\///\\\__________\/\\\_____________\/\\\_______\/\\\____\//\\\\\\//\\\\\_____
       ____\////\\\\\\\\\_\/\\\\\\\\\\\\\\\_\/\\\_______\/\\\_____\//\\\__\//\\\______
        _______\/////////__\///////////////__\///________\///_______\///____\///_______"#;

    let logo_lines: Vec<Line> = logo_text
        .lines()
        .map(|line| {
            let spans: Vec<Span> = line
                .chars()
                .map(|ch| {
                    if ch == '_' {
                        Span::styled(ch.to_string(), Style::default().fg(soft_neon_green))
                    } else {
                        Span::styled(ch.to_string(), Style::default().fg(bright_orange))
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect();

    let logo = Paragraph::new(logo_lines).style(Style::default());
    frame.render_widget(logo, area);
}

/// Renders the selection mode (dual-panel view).
fn render_selection_mode(frame: &mut Frame, app: &GoalBrowserApp) {
    let area = frame.area();

    // Create vertical layout: logo + main area + help footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9), // Logo area (9 lines)
            Constraint::Min(3),    // Main area
            Constraint::Length(3), // Help footer
        ])
        .split(area);

    let logo_area = chunks[0];
    let main_area = chunks[1];
    let help_area = chunks[2];

    // Render logo
    render_logo(logo_area, frame);

    // Determine which panels to show
    let show_local = !app.local_goals.is_empty();
    let show_global = !app.global_goals.is_empty();

    // Create horizontal layout based on which panels we need
    let panels = if show_local && show_global {
        // Show both panels
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_area)
    } else if show_local || show_global {
        // Show single panel (full width)
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(main_area)
    } else {
        // No goals at all (shouldn't happen, but handle gracefully)
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(main_area)
    };

    // Render panels
    if show_local && show_global {
        render_goal_panel(frame, panels[0], &app.local_goals, app.local_selected, "Local Goals", app.active_panel == Panel::Local);
        render_goal_panel(frame, panels[1], &app.global_goals, app.global_selected, "Global Goals", app.active_panel == Panel::Global);
    } else if show_local {
        render_goal_panel(frame, panels[0], &app.local_goals, app.local_selected, "Local Goals", true);
    } else if show_global {
        render_goal_panel(frame, panels[0], &app.global_goals, app.global_selected, "Global Goals", true);
    }

    // Render help footer
    render_help_footer(frame, help_area);
}

/// Renders a single goal panel.
fn render_goal_panel(
    frame: &mut Frame,
    area: Rect,
    goals: &[DiscoveredGoal],
    selected: usize,
    title: &str,
    is_active: bool,
) {
    // Create list items from goals
    let items: Vec<ListItem> = goals
        .iter()
        .enumerate()
        .map(|(i, goal)| {
            let description = goal
                .config
                .description
                .as_deref()
                .unwrap_or("No description");

            // Format: {name} ({folder_name}) -- {description}
            let content = format!(
                "{} ({}) -- {}",
                goal.config.name, goal.name, description
            );

            // Highlight selected item
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(if is_active { Color::Cyan } else { Color::DarkGray })
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    // Create the list widget
    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if is_active {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            }),
    );

    frame.render_widget(list, area);
}

/// Renders the help footer with keybindings.
fn render_help_footer(frame: &mut Frame, area: Rect) {
    let orange = Color::Rgb(255, 165, 0);
    let help_text = vec![
        Line::from(vec![
            Span::styled("↑/↓ or j/k", Style::default().fg(orange)),
            Span::raw(": Navigate  "),
            Span::styled("Tab", Style::default().fg(orange)),
            Span::raw(": Switch Panel  "),
            Span::styled("v", Style::default().fg(orange)),
            Span::raw(": View  "),
            Span::styled("Enter", Style::default().fg(orange)),
            Span::raw(": Select  "),
            Span::styled("Esc/q", Style::default().fg(orange)),
            Span::raw(": Quit"),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help, area);
}

/// Renders the view mode (prompt.yaml preview).
fn render_view_mode(frame: &mut Frame, app: &GoalBrowserApp) {
    let area = frame.area();

    // Create vertical layout: header + content area + help footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with file path
            Constraint::Min(3),    // Content area
            Constraint::Length(3), // Help footer
        ])
        .split(area);

    let header_area = chunks[0];
    let content_area = chunks[1];
    let help_area = chunks[2];

    // Render header with file path
    if let Some(path) = &app.view_path {
        let header = Paragraph::new(path.as_str())
            .block(
                Block::default()
                    .title("Viewing")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White));
        frame.render_widget(header, header_area);
    }

    // Render content
    if let Some(content) = &app.view_content {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Clamp scroll to valid range
        let max_scroll = total_lines.saturating_sub(content_area.height as usize - 2); // -2 for borders
        let scroll = app.view_scroll.min(max_scroll);

        // Get visible lines
        let visible_lines: Vec<Line> = lines
            .iter()
            .skip(scroll)
            .take(content_area.height as usize - 2)
            .map(|line| Line::from(*line))
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .title(format!(
                        "Content (line {}/{}) - Use ↑/↓ to scroll, Esc to exit",
                        scroll + 1,
                        total_lines
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, content_area);
    }

    // Render help footer for view mode
    let orange = Color::Rgb(255, 165, 0);
    let help_text = vec![Line::from(vec![
        Span::styled("↑/↓ or j/k", Style::default().fg(orange)),
        Span::raw(": Scroll  "),
        Span::styled("PgUp/PgDn", Style::default().fg(orange)),
        Span::raw(": Page  "),
        Span::styled("Esc/q", Style::default().fg(orange)),
        Span::raw(": Back"),
    ])];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help, help_area);
}

/// Handles keyboard input and updates application state.
fn handle_input(key: KeyEvent, app: &mut GoalBrowserApp) -> Result<ControlFlow> {
    match app.mode {
        AppMode::Selection => handle_selection_input(key, app),
        AppMode::ViewMode => handle_view_input(key, app),
    }
}

/// Handles input in selection mode.
fn handle_selection_input(key: KeyEvent, app: &mut GoalBrowserApp) -> Result<ControlFlow> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Ok(ControlFlow::Quit),
        KeyCode::Enter => Ok(ControlFlow::Select),
        KeyCode::Tab => {
            app.toggle_panel();
            Ok(ControlFlow::Continue)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            Ok(ControlFlow::Continue)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            Ok(ControlFlow::Continue)
        }
        KeyCode::Char('v') => {
            app.enter_view_mode()?;
            Ok(ControlFlow::Continue)
        }
        _ => Ok(ControlFlow::Continue),
    }
}

/// Handles input in view mode.
fn handle_view_input(key: KeyEvent, app: &mut GoalBrowserApp) -> Result<ControlFlow> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            // Exit view mode back to selection
            app.mode = AppMode::Selection;
            app.view_content = None;
            app.view_path = None;
            app.view_scroll = 0;
            Ok(ControlFlow::Continue)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_up();
            Ok(ControlFlow::Continue)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_down();
            Ok(ControlFlow::Continue)
        }
        KeyCode::PageUp => {
            app.page_up(10);
            Ok(ControlFlow::Continue)
        }
        KeyCode::PageDown => {
            app.page_down(10);
            Ok(ControlFlow::Continue)
        }
        _ => Ok(ControlFlow::Continue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GoalSource, PromptConfig};
    use std::collections::HashMap;

    fn create_test_goal(name: &str, source: GoalSource) -> DiscoveredGoal {
        DiscoveredGoal {
            name: name.to_string(),
            source,
            config: PromptConfig {
                name: format!("{} Display Name", name),
                description: Some(format!("{} description", name)),
                parameters: Vec::new(),
                context_scripts: HashMap::new(),
                prompt: "test prompt".to_string(),
            },
        }
    }

    #[test]
    fn test_new_app_with_only_local_goals() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("local2", GoalSource::Local),
        ];

        let app = GoalBrowserApp::new(goals);

        assert_eq!(app.local_goals.len(), 2);
        assert_eq!(app.global_goals.len(), 0);
        assert_eq!(app.active_panel, Panel::Local);
        assert_eq!(app.local_selected, 0);
    }

    #[test]
    fn test_new_app_with_only_global_goals() {
        let goals = vec![
            create_test_goal("global1", GoalSource::Global),
            create_test_goal("global2", GoalSource::Global),
        ];

        let app = GoalBrowserApp::new(goals);

        assert_eq!(app.local_goals.len(), 0);
        assert_eq!(app.global_goals.len(), 2);
        assert_eq!(app.active_panel, Panel::Global);
        assert_eq!(app.global_selected, 0);
    }

    #[test]
    fn test_new_app_with_mixed_goals() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("global1", GoalSource::Global),
        ];

        let app = GoalBrowserApp::new(goals);

        assert_eq!(app.local_goals.len(), 1);
        assert_eq!(app.global_goals.len(), 1);
        assert_eq!(app.active_panel, Panel::Local);
    }

    #[test]
    fn test_move_up_at_top() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("local2", GoalSource::Local),
        ];

        let mut app = GoalBrowserApp::new(goals);
        assert_eq!(app.local_selected, 0);

        app.move_up();
        assert_eq!(app.local_selected, 0); // Should stay at 0
    }

    #[test]
    fn test_move_down_at_bottom() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("local2", GoalSource::Local),
        ];

        let mut app = GoalBrowserApp::new(goals);
        app.local_selected = 1;

        app.move_down();
        assert_eq!(app.local_selected, 1); // Should stay at 1 (last item)
    }

    #[test]
    fn test_move_up_and_down() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("local2", GoalSource::Local),
            create_test_goal("local3", GoalSource::Local),
        ];

        let mut app = GoalBrowserApp::new(goals);
        assert_eq!(app.local_selected, 0);

        app.move_down();
        assert_eq!(app.local_selected, 1);

        app.move_down();
        assert_eq!(app.local_selected, 2);

        app.move_up();
        assert_eq!(app.local_selected, 1);
    }

    #[test]
    fn test_toggle_panel_with_both_goals() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("global1", GoalSource::Global),
        ];

        let mut app = GoalBrowserApp::new(goals);
        assert_eq!(app.active_panel, Panel::Local);

        app.toggle_panel();
        assert_eq!(app.active_panel, Panel::Global);

        app.toggle_panel();
        assert_eq!(app.active_panel, Panel::Local);
    }

    #[test]
    fn test_toggle_panel_with_only_local() {
        let goals = vec![create_test_goal("local1", GoalSource::Local)];

        let mut app = GoalBrowserApp::new(goals);
        assert_eq!(app.active_panel, Panel::Local);

        app.toggle_panel();
        assert_eq!(app.active_panel, Panel::Local); // Should not switch
    }

    #[test]
    fn test_get_selected_goal() {
        let goals = vec![
            create_test_goal("local1", GoalSource::Local),
            create_test_goal("global1", GoalSource::Global),
        ];

        let mut app = GoalBrowserApp::new(goals);

        let selected = app.get_selected_goal().unwrap();
        assert_eq!(selected.name, "local1");

        app.toggle_panel();
        let selected = app.get_selected_goal().unwrap();
        assert_eq!(selected.name, "global1");
    }

    #[test]
    fn test_scroll_up_at_top() {
        let goals = vec![create_test_goal("local1", GoalSource::Local)];
        let mut app = GoalBrowserApp::new(goals);

        app.view_scroll = 0;
        app.scroll_up();
        assert_eq!(app.view_scroll, 0); // Should stay at 0
    }

    #[test]
    fn test_scroll_down() {
        let goals = vec![create_test_goal("local1", GoalSource::Local)];
        let mut app = GoalBrowserApp::new(goals);

        app.view_scroll = 0;
        app.scroll_down();
        assert_eq!(app.view_scroll, 1);

        app.scroll_down();
        assert_eq!(app.view_scroll, 2);
    }

    #[test]
    fn test_page_up_and_down() {
        let goals = vec![create_test_goal("local1", GoalSource::Local)];
        let mut app = GoalBrowserApp::new(goals);

        app.view_scroll = 20;
        app.page_up(10);
        assert_eq!(app.view_scroll, 10);

        app.page_down(5);
        assert_eq!(app.view_scroll, 15);
    }

    #[test]
    fn test_page_up_underflow() {
        let goals = vec![create_test_goal("local1", GoalSource::Local)];
        let mut app = GoalBrowserApp::new(goals);

        app.view_scroll = 5;
        app.page_up(10);
        assert_eq!(app.view_scroll, 0); // Should not underflow
    }
}
