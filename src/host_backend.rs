//! The backend cozy draws through: crossterm's, except for the size.
//!
//! ratatui asks the backend for the terminal's size while it clears the screen
//! after a resize (`clear_fixed_viewport` calls `backend.size()`). Crossterm
//! answers that by asking the tty — and a host that embeds cozy (argotty on a
//! phone) has none, so the question fails. `Terminal::resize` then returns before
//! it clears the screen or forgets what it drew, and from there on it writes only
//! the cells it believes changed: whatever the smaller screen showed stays where
//! it was (the soft keyboard is enough to trigger it).
//!
//! A host that hands in a size is the authority on it, so this backend answers
//! with that size and never asks the tty. Without one (the CLI) it asks crossterm
//! as before.

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use std::io::{self, Write};

pub struct HostBackend<W: Write> {
    inner: CrosstermBackend<W>,
    /// The size the host handed in; `None` means "ask the terminal".
    size: Option<Size>,
}

impl<W: Write> HostBackend<W> {
    pub fn new(writer: W, size: Option<(u16, u16)>) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            size: size.map(|(width, height)| Size { width, height }),
        }
    }

    /// The host says the terminal is now `cols` × `rows`. Call it before
    /// `Terminal::resize`, which asks this backend for the size while clearing.
    /// Without a host size there is nothing to note: the terminal is asked.
    pub fn note_size(&mut self, cols: u16, rows: u16) {
        if self.size.is_some() {
            self.size = Some(Size {
                width: cols,
                height: rows,
            });
        }
    }
}

impl<W: Write> Backend for HostBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        match self.size {
            Some(size) => Ok(size),
            None => self.inner.size(),
        }
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        match self.size {
            // No pixel size to give: the host passes cell pixels separately
            // (`ImageCaps::cell_px`), and nothing here reads them.
            Some(size) => Ok(WindowSize {
                columns_rows: size,
                pixels: Size::default(),
            }),
            None => self.inner.window_size(),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

impl<W: Write> Write for HostBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::widgets::Paragraph;
    use ratatui::{Terminal, TerminalOptions, Viewport};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// What the backend wrote, readable while the terminal still holds it.
    #[derive(Clone, Default)]
    struct Shared(Rc<RefCell<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_host_size_is_answered_without_asking_the_terminal() {
        let mut backend = HostBackend::new(Vec::new(), Some((80, 24)));
        assert_eq!(backend.size().unwrap(), Size::new(80, 24));
        backend.note_size(100, 30);
        assert_eq!(backend.size().unwrap(), Size::new(100, 30));
    }

    /// The soft keyboard: shrink, then grow back. With the size answered,
    /// `resize` succeeds and clears what the small screen showed.
    #[test]
    fn a_resize_clears_the_screen_when_the_host_gave_the_size() {
        let out = Shared::default();
        let mut terminal = Terminal::with_options(
            HostBackend::new(out.clone(), Some((20, 10))),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 20, 10)),
            },
        )
        .unwrap();
        terminal
            .draw(|f| f.render_widget(Paragraph::new("first"), f.area()))
            .unwrap();
        out.0.borrow_mut().clear();

        terminal.backend_mut().note_size(20, 5);
        terminal.resize(Rect::new(0, 0, 20, 5)).expect("resize");
        terminal.backend_mut().note_size(20, 10);
        terminal.resize(Rect::new(0, 0, 20, 10)).expect("resize");

        let written = String::from_utf8_lossy(&out.0.borrow()).into_owned();
        // ClearType::AfterCursor from the viewport's top-left (CSI J / CSI 0J).
        assert!(
            written.contains("\x1b[J") || written.contains("\x1b[0J"),
            "no clear was written: {written:?}"
        );
    }
}
