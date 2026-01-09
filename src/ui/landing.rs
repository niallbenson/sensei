//! Landing screen with animated ASCII art

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    widgets::Paragraph,
};

use crate::app::state::LandingAnimation;
use crate::theme::Theme;

/// ASCII art logo for Sensei
const LOGO: &str = r#"
   _____ ______ _   _ _____ ______ _____
  / ____|  ____| \ | / ____|  ____|_   _|
 | (___ | |__  |  \| | (___ | |__    | |
  \___ \|  __| | . ` |\___ \|  __|   | |
  ____) | |____| |\  |____) | |____ _| |_
 |_____/|______|_| \_|_____/|______|_____|

"#;

const TAGLINE: &str = "Your AI-powered guide to mastering technical books";
const PROMPT: &str = "Press any key to begin...";

/// Draw the landing screen with animation
pub fn draw(frame: &mut Frame, animation: &LandingAnimation, theme: &Theme) {
    let area = frame.area();

    // Fill background
    let bg_style = Style::default().bg(theme.bg_primary);
    frame.render_widget(Paragraph::new("").style(bg_style), area);

    // Center the content vertically - calculate positions manually
    let content_start_y = area.height / 3;

    // Get the portion of logo to display based on animation progress
    let logo_chars: Vec<char> = LOGO.chars().collect();
    let visible_chars = animation.current_char.min(logo_chars.len());
    let visible_logo: String = logo_chars[..visible_chars].iter().collect();

    // Style for the logo
    let logo_style = Style::default().fg(theme.accent_primary).bg(theme.bg_primary);

    let logo = Paragraph::new(visible_logo).style(logo_style).alignment(Alignment::Center);

    // Calculate centered position for logo
    let logo_area = center_rect(area, 50, 8);
    let logo_area = Rect { y: content_start_y, ..logo_area };
    frame.render_widget(logo, logo_area);

    // Only show tagline and prompt after logo animation completes
    if animation.current_char >= logo_chars.len() {
        let tagline_style = Style::default().fg(theme.fg_secondary).bg(theme.bg_primary);

        let tagline = Paragraph::new(TAGLINE).style(tagline_style).alignment(Alignment::Center);

        let tagline_area =
            Rect { x: area.x, y: logo_area.y + logo_area.height + 2, width: area.width, height: 1 };
        frame.render_widget(tagline, tagline_area);

        // Blinking prompt
        let blink = (animation.start_time.elapsed().as_millis() / 500) % 2 == 0;
        if blink {
            let prompt_style = Style::default().fg(theme.fg_muted).bg(theme.bg_primary);

            let prompt = Paragraph::new(PROMPT).style(prompt_style).alignment(Alignment::Center);

            let prompt_area =
                Rect { x: area.x, y: tagline_area.y + 3, width: area.width, height: 1 };
            frame.render_widget(prompt, prompt_area);
        }
    }
}

/// Center a rect within another rect
fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width: width.min(area.width), height: height.min(area.height) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_has_content() {
        assert!(LOGO.len() > 100); // Logo should have substantial content
    }

    #[test]
    fn center_rect_works() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = center_rect(area, 20, 10);
        assert_eq!(centered.x, 40);
        assert_eq!(centered.y, 20);
    }
}
