//! Message boxes (`MessageBox`, `cAlert` `0x410b5b`) in Windows 95's look:
//! a caption bar, the text, buttons. The file dialogs are
//! [`super::file_dialog`].

use crate::assets::Assets;
use crate::gfx::{IRect, Surface};

use super::ui;

/// A message box's answer (`IDOK`, `IDCANCEL`, `IDYES`, `IDNO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Ok,
    Cancel,
    Yes,
    No,
}

impl Answer {
    fn label(self) -> &'static str {
        match self {
            Answer::Ok => "OK",
            Answer::Cancel => "Cancel",
            Answer::Yes => "&Yes",
            Answer::No => "&No",
        }
    }
}

const CAPTION_H: i32 = 18;
const BUTTON_W: i32 = 75;
const BUTTON_H: i32 = 23;
const MARGIN: i32 = 12;

/// A modal box over the editor.
#[derive(Debug, Clone)]
pub struct Prompt {
    pub title: String,
    pub text: String,
    pub buttons: Vec<Answer>,
    /// The button the mouse holds down.
    pressed: Option<usize>,
}

impl Prompt {
    #[must_use]
    pub fn new(title: &str, text: &str, buttons: &[Answer]) -> Prompt {
        Prompt {
            title: title.to_owned(),
            text: text.to_owned(),
            buttons: buttons.to_vec(),
            pressed: None,
        }
    }

    fn layout(&self, assets: &Assets, area: IRect) -> (IRect, Vec<String>, Vec<IRect>) {
        let Some(font) = ui::font(assets) else {
            return (IRect::default(), Vec::new(), Vec::new());
        };
        let lines = ui::wrap(font, &self.text, 360);
        let tw = lines.iter().map(|l| ui::width(font, l)).max().unwrap_or(0);
        let nb = i32::try_from(self.buttons.len()).unwrap_or(0);
        let bw = nb * BUTTON_W + (nb - 1).max(0) * 6;
        let w = (tw + 2 * MARGIN)
            .max(bw + 2 * MARGIN)
            .max(ui::width(font, &self.title) + 40);
        let lh = i32::from(font.height);
        let text_h = lh * i32::try_from(lines.len()).unwrap_or(0);
        let h = CAPTION_H + 3 + MARGIN + text_h + 8 + BUTTON_H + MARGIN + 3;
        let w = w + 6;
        let r = IRect::sized(
            area.x0 + (area.width() - w) / 2,
            area.y0 + (area.height() - h) / 2,
            w,
            h,
        );
        let by = r.y1 - 3 - MARGIN - BUTTON_H;
        let bx = r.x0 + (r.width() - bw) / 2;
        let buttons = (0..nb)
            .map(|i| IRect::sized(bx + i * (BUTTON_W + 6), by, BUTTON_W, BUTTON_H))
            .collect();
        (r, lines, buttons)
    }

    /// Draws the box centred in `area`.
    pub fn draw(&self, s: &mut Surface, assets: &Assets, area: IRect) {
        let Some(font) = ui::font(assets) else { return };
        let (r, lines, buttons) = self.layout(assets, area);
        s.fill(r, ui::FACE);
        ui::raised(s, r);
        let cap = IRect::new(r.x0 + 3, r.y0 + 3, r.x1 - 3, r.y0 + 3 + CAPTION_H);
        s.fill(cap, ui::NAVY);
        s.text(
            font,
            cap.x0 + 3,
            cap.y0 + 2,
            &crate::text::encode(&self.title),
            ui::WHITE,
        );
        let mut y = cap.y1 + MARGIN;
        for l in &lines {
            s.text(font, r.x0 + 3 + MARGIN, y, &crate::text::encode(l), ui::BLACK);
            y += i32::from(font.height);
        }
        for (i, (b, rect)) in self.buttons.iter().zip(buttons).enumerate() {
            ui::button(s, font, rect, b.label(), self.pressed == Some(i), i == 0, true);
        }
    }

    /// The mouse went down at `p`.
    pub fn mouse_down(&mut self, assets: &Assets, area: IRect, p: (i32, i32)) {
        let (_, _, buttons) = self.layout(assets, area);
        self.pressed = buttons.iter().position(|b| b.contains(p.0, p.1));
    }

    /// The mouse went up at `p`: the answer if it's on the pressed button.
    pub fn mouse_up(&mut self, assets: &Assets, area: IRect, p: (i32, i32)) -> Option<Answer> {
        let (_, _, buttons) = self.layout(assets, area);
        let i = self.pressed.take()?;
        buttons[i].contains(p.0, p.1).then(|| self.buttons[i])
    }

    /// A key: Enter is the first button, Esc Cancel (or OK alone),
    /// letters the buttons' mnemonics.
    #[must_use]
    pub fn key(&self, vk: u16) -> Option<Answer> {
        match vk {
            0x0d => return self.buttons.first().copied(),
            0x1b => {
                return self
                    .buttons
                    .iter()
                    .copied()
                    .find(|&b| b == Answer::Cancel)
                    .or_else(|| (self.buttons == [Answer::Ok]).then_some(Answer::Ok));
            }
            _ => {}
        }
        let c = char::from(u8::try_from(vk).ok()?);
        self.buttons
            .iter()
            .copied()
            .find(|b| ui::mnemonic(b.label()) == Some(c))
    }
}

/// The character a key types on a US keyboard.
#[must_use]
pub fn typed(vk: u16, shift: bool) -> Option<char> {
    let c = match (vk, shift) {
        (0x41..=0x5a, false) => char::from(u8::try_from(vk).ok()? + 32),
        (0x41..=0x5a, true) | (0x30..=0x39, false) => char::from(u8::try_from(vk).ok()?),
        (0x30..=0x39, true) => *b")!@#$%^&*("
            .get(usize::from(vk - 0x30))
            .map(|&b| char::from(b))
            .as_ref()?,
        (0x60..=0x69, _) => char::from(u8::try_from(vk - 0x30).ok()?),
        (0x20, _) => ' ',
        (0xbe | 0x6e, false) => '.',
        (0xbc, false) => ',',
        (0xbd | 0x6d, false) => '-',
        (0xbd, true) => '_',
        (0xbf | 0x6f, false) => '/',
        (0xdc, false) => '\\',
        (0xba, true) => ':',
        (0xba, false) => ';',
        (0xde, false) => '\'',
        (0xbb, false) => '=',
        (0xbb, true) | (0x6b, _) => '+',
        (0xdb, false) => '[',
        (0xdd, false) => ']',
        (0xc0, true) => '~',
        _ => return None,
    };
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_and_typing() {
        let p = Prompt::new("t", "x", &[Answer::Yes, Answer::No, Answer::Cancel]);
        assert_eq!(p.key(u16::from(b'N')), Some(Answer::No));
        assert_eq!(p.key(0x0d), Some(Answer::Yes));
        assert_eq!(p.key(0x1b), Some(Answer::Cancel));
        assert_eq!(Prompt::new("t", "x", &[Answer::Ok]).key(0x1b), Some(Answer::Ok));
        assert_eq!(typed(0x42, false), Some('b'));
        assert_eq!(typed(0x57, true), Some('W'));
        assert_eq!(typed(0xbe, false), Some('.'));
    }
}
