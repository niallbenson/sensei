//! Quiz panel overlay component

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::state::AppState;
use crate::theme::Theme;

/// Draw the quiz panel as a centered overlay
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    // Don't draw if quiz is not active
    if !state.quiz.active {
        return;
    }

    // Calculate centered overlay area (70% width, 70% height)
    let overlay_area = centered_rect(70, 70, area);

    // Clear the background area
    frame.render_widget(Clear, overlay_area);

    // Determine title based on state
    let title = if state.quiz.loading {
        " Generating Quiz... "
    } else if state.quiz.completed {
        " Quiz Results "
    } else if state.quiz.error.is_some() {
        " Quiz Error "
    } else {
        " Quiz "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .style(Style::default().bg(theme.bg_secondary));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    // Draw content based on state
    if state.quiz.loading {
        draw_loading(frame, inner, theme);
    } else if let Some(ref error) = state.quiz.error {
        draw_error(frame, inner, error, theme);
    } else if state.quiz.completed {
        draw_results(frame, inner, state, theme);
    } else {
        draw_question(frame, inner, state, theme);
    }
}

/// Draw loading state
fn draw_loading(frame: &mut Frame, area: Rect, theme: &Theme) {
    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Generating quiz questions...",
            Style::default().fg(theme.fg_primary),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Please wait while Claude creates your quiz.",
            Style::default().fg(theme.fg_muted),
        )),
    ];

    let para = Paragraph::new(text).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(para, area);
}

/// Draw error state
fn draw_error(frame: &mut Frame, area: Rect, error: &str, theme: &Theme) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Failed to generate quiz",
            Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(error, Style::default().fg(theme.fg_secondary))),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "[Enter] Retry    [Esc] Close",
            Style::default().fg(theme.fg_muted),
        )),
    ];

    let para = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(para, area);
}

/// Draw results screen
fn draw_results(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let (correct, total) = state.quiz.score();
    let passed = state.quiz.passed();

    let mut lines = vec![Line::from(""), Line::from("")];

    // Result header
    if passed {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("Passed! {}/{} correct", correct, total),
                Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{}/{} correct - Need 100% to pass", correct, total),
                Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Question results
    let mut result_spans = vec![Span::raw("  ")];
    for (i, question) in state.quiz.questions.iter().enumerate() {
        let user_answer = state.quiz.answers.get(i).copied().flatten();
        let is_correct = user_answer == Some(question.correct_index);

        let marker = if is_correct { " \u{2713} " } else { " \u{2717} " }; // ✓ or ✗
        let style = if is_correct {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.error)
        };

        result_spans.push(Span::styled(format!("Q{}{}", i + 1, marker), style));
    }
    lines.push(Line::from(result_spans));

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Action hint
    if passed {
        lines.push(Line::from(Span::styled(
            "[Enter] Continue to Next Section",
            Style::default().fg(theme.fg_muted),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "[Enter] Retry    [Esc] Back to Section",
            Style::default().fg(theme.fg_muted),
        )));
    }

    let para = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(para, area);
}

/// Draw current question
fn draw_question(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let current = state.quiz.current_question;
    let total = state.quiz.questions.len();

    let Some(question) = state.quiz.questions.get(current) else {
        return;
    };

    let mut lines = vec![];

    // Question number
    lines.push(Line::from(Span::styled(
        format!("Question {} of {}", current + 1, total),
        Style::default().fg(theme.fg_muted),
    )));
    lines.push(Line::from(""));

    // Question text
    lines.push(Line::from(Span::styled(
        &question.question,
        Style::default().fg(theme.fg_primary).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Options
    for (i, option) in question.options.iter().enumerate() {
        let is_selected = i == state.quiz.selected_option;
        let prefix = if is_selected { "\u{25CF}" } else { "\u{25CB}" }; // ● or ○
        let letter = (b'A' + i as u8) as char;

        let style = if is_selected {
            Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg_secondary)
        };

        lines.push(Line::from(Span::styled(format!("  {} {}) {}", prefix, letter, option), style)));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));

    // Hint
    lines.push(Line::from(Span::styled(
        "[j/k] Select    [Enter] Confirm    [Esc] Cancel",
        Style::default().fg(theme.fg_muted),
    )));

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, area);
}

/// Create a centered rectangle with the given percentage of width and height
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
