mod geometry;
mod render;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::ResetColor,
    terminal::{
        self, Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use geometry::Dodecahedron;
use render::Canvas;
use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
    time::{Duration, Instant},
};

const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// Restores terminal settings on a normal exit, an I/O error, or unwinding.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let guard = Self;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            Hide,
            DisableLineWrap,
            Clear(ClearType::All)
        )?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            ResetColor,
            EnableLineWrap,
            Show,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

fn cell_aspect() -> f64 {
    // Some terminals supply the window's pixel size through TIOCGWINSZ.
    // Otherwise assume the usual approximately 1:2 character cell.
    if let Ok(size) = terminal::window_size()
        && size.columns > 0
        && size.rows > 0
        && size.width > 0
        && size.height > 0
    {
        let aspect =
            (size.height as f64 / size.rows as f64) / (size.width as f64 / size.columns as f64);
        if (0.5..=4.0).contains(&aspect) {
            return aspect;
        }
    }
    2.0
}

fn animate() -> io::Result<()> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        return Err(io::Error::other(
            "Для анимации нужен терминал. Для вывода в файл: ascii-dodecahedron --snapshot 80 24",
        ));
    }
    let solid = Dodecahedron::new();
    let _terminal = TerminalGuard::enter()?;
    let mut stdout = io::stdout().lock();
    let mut canvas = Canvas::new(0, 0);
    let mut output = String::new();
    let mut previous = Instant::now();
    let mut time = 0.0;
    let mut paused = false;

    loop {
        let frame_start = Instant::now();
        if !paused {
            time += frame_start.duration_since(previous).as_secs_f64();
        }
        previous = frame_start;
        let (columns, rows) = terminal::size()?;
        let width = columns.max(1) as usize;
        let height = rows.saturating_sub(1) as usize;
        if canvas.width != width || canvas.height != height {
            canvas = Canvas::new(width, height);
            execute!(stdout, Clear(ClearType::All))?;
        }
        canvas.render(&solid, time, cell_aspect());
        output.clear();
        output.push_str("\x1b[H");
        if height > 0 {
            canvas.write_ansi(&mut output);
            output.push_str("\r\n");
        }
        output.push_str("\x1b[0;37;40m");
        let status = format!(
            " {columns}x{rows} | Q/Esc: quit | Space: pause | R: reset{}",
            if paused { " | PAUSED" } else { "" }
        );
        // The status line is ASCII too, so byte length equals terminal width.
        let shown = &status[..status.len().min(width)];
        output.push_str(shown);
        output.extend(std::iter::repeat_n(' ', width - shown.len()));
        output.push_str("\x1b[0m");
        stdout.write_all(output.as_bytes())?;
        stdout.flush()?;

        if event::poll(FRAME_TIME.saturating_sub(frame_start.elapsed()))? {
            loop {
                if let Event::Key(key) = event::read()?
                    && key.kind != KeyEventKind::Release
                {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q' | 'Q') => return Ok(()),
                        KeyCode::Char('c' | 'C')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }
                        KeyCode::Char(' ') => paused = !paused,
                        KeyCode::Char('r' | 'R') => time = 0.0,
                        _ => {}
                    }
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }
}

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => animate(),
        [help] if help == "--help" || help == "-h" => {
            println!(
                "Цветной ASCII-додекаэдр\n\n  ascii-dodecahedron                     анимация, размер окна определяется автоматически\n  ascii-dodecahedron --snapshot 80 24    один цветной кадр (ширина и высота в символах)\n\nQ / Esc / Ctrl+C — выход; Пробел — пауза; R — сброс вращения.\nРекомендуется терминал с поддержкой 24-битного цвета и моноширинный шрифт."
            );
            Ok(())
        }
        [flag, width, height] if flag == "--snapshot" => {
            let dimension = |value: &str, max: usize| -> io::Result<usize> {
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0 && n <= max)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("Размер должен быть целым числом от 1 до {max}"),
                        )
                    })
            };
            let mut canvas = Canvas::new(dimension(width, 1000)?, dimension(height, 500)?);
            canvas.render(&Dodecahedron::new(), 0.0, 2.0);
            let mut output = String::new();
            canvas.write_ansi(&mut output);
            output.push('\n');
            io::stdout().lock().write_all(output.as_bytes())
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Неизвестные аргументы. Справка: ascii-dodecahedron --help",
        )),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Ошибка: {error}");
            ExitCode::FAILURE
        }
    }
}
