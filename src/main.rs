use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;
use std::process::Command;
use sysinfo::System;

/// DevPort - Development Port Manager TUI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Additional ports to monitor (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    ports: Vec<u16>,

    /// List all monitored ports and exit
    #[arg(short, long)]
    list: bool,

    /// Scan ports and show what's running (debug mode)
    #[arg(short, long)]
    scan: bool,
}

// Common development ports to check
const DEFAULT_PORTS: &[u16] = &[
    3000, 3001, 3002, 3003, // Frontend dev servers (React/Next.js/Vite)
    4200, // Nx frontend (Angular/React/Vue)
    5000, 5001, 5173, 8000, 8080, 8081, // Backend/API servers
    5432,  // PostgreSQL
    3306,  // MySQL
    6379,  // Redis
    27017, // MongoDB
    9229,  // Node debugger
];

#[derive(Clone, Debug)]
struct PortProcess {
    port: u16,
    pid: i32,
    name: String,
    command: String,
    command_full: String,
    description: String,
}

struct App {
    processes: Vec<PortProcess>,
    list_state: ListState,
    message: Option<String>,
    ports: Vec<u16>,
}

impl App {
    fn new(custom_ports: Vec<u16>) -> Self {
        let mut ports = DEFAULT_PORTS.to_vec();
        ports.extend(custom_ports);
        ports.sort();
        ports.dedup();

        let mut app = App {
            processes: Vec::new(),
            list_state: ListState::default(),
            message: None,
            ports,
        };
        app.refresh();
        if !app.processes.is_empty() {
            app.list_state.select(Some(0));
        }
        app
    }

    fn refresh(&mut self) {
        self.processes = scan_ports(&self.ports);
        if self.processes.is_empty() {
            self.list_state.select(None);
        } else if let Some(selected) = self.list_state.selected() {
            if selected >= self.processes.len() {
                self.list_state.select(Some(0));
            }
        }
    }

    fn next(&mut self) {
        if self.processes.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.processes.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.processes.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.processes.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn kill_selected(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if let Some(process) = self.processes.get(i) {
                let pid = process.pid;
                match kill_process(pid) {
                    Ok(_) => {
                        self.message = Some(format!("Killed PID {} successfully", pid));
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        self.refresh();
                    }
                    Err(e) => {
                        self.message = Some(format!("Failed to kill PID {}: {}", pid, e));
                    }
                }
            }
        }
    }
}

fn scan_ports(ports: &[u16]) -> Vec<PortProcess> {
    scan_ports_debug(ports, false)
}

fn scan_ports_debug(ports: &[u16], debug: bool) -> Vec<PortProcess> {
    let mut results = Vec::new();

    for &port in ports {
        if debug {
            eprintln!("Checking port {}...", port);
        }
        let port_arg = format!("-iTCP:{}", port);
        match Command::new("/usr/sbin/lsof")
            .args(&[&port_arg, "-sTCP:LISTEN", "-P", "-n"])
            .output()
        {
            Ok(output) => {
                if debug {
                    eprintln!("  lsof exit code: {:?}", output.status.code());
                    eprintln!("  stdout length: {}", output.stdout.len());
                    if !output.stderr.is_empty() {
                        eprintln!("  stderr: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if debug && !stdout.is_empty() {
                        eprintln!("  stdout: {}", stdout);
                    }
                    for line in stdout.lines().skip(1) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let name = parts[0].to_string();
                            if let Ok(pid) = parts[1].parse::<i32>() {
                                let command_full = get_process_command_full(pid);
                                let command = get_process_command(pid);
                                let description = infer_description(port, &name, &command_full);
                                if debug {
                                    eprintln!("  Found: {} PID {} - {}", name, pid, description);
                                }
                                results.push(PortProcess {
                                    port,
                                    pid,
                                    name: name.clone(),
                                    command: command.clone(),
                                    command_full: command_full.clone(),
                                    description,
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if debug {
                    eprintln!("  Error running lsof: {}", e);
                }
            }
        }
    }

    results.sort_by_key(|p| p.port);
    results
}

fn get_service_emoji(description: &str) -> &'static str {
    let desc_lower = description.to_lowercase();
    if desc_lower.contains("vite") || desc_lower.contains("react") || desc_lower.contains("next") {
        "⚡"
    } else if desc_lower.contains("nx") || desc_lower.contains("angular") {
        "🔷"
    } else if desc_lower.contains("node") || desc_lower.contains("api") || desc_lower.contains("express") {
        "🟢"
    } else if desc_lower.contains("python") || desc_lower.contains("flask") || desc_lower.contains("django") || desc_lower.contains("fastapi") {
        "🐍"
    } else if desc_lower.contains("postgres") {
        "🐘"
    } else if desc_lower.contains("mysql") {
        "🐬"
    } else if desc_lower.contains("redis") {
        "🔴"
    } else if desc_lower.contains("mongo") {
        "🍃"
    } else if desc_lower.contains("docker") {
        "🐳"
    } else if desc_lower.contains("macos") || desc_lower.contains("control") {
        "🍎"
    } else if desc_lower.contains("debug") {
        "🐛"
    } else {
        "🔹"
    }
}

fn infer_description(port: u16, process_name: &str, command: &str) -> String {
    let cmd_lower = command.to_lowercase();
    let name_lower = process_name.to_lowercase();

    // Extract project/directory name from command
    let extract_project = |cmd: &str| -> Option<String> {
        // Look for common project path patterns and get the deepest project name
        let mut best_match: Option<String> = None;

        // First try to find node_modules or similar markers
        if let Some(idx) = cmd.find("/node_modules/") {
            // Get the directory before node_modules
            let before = &cmd[..idx];
            if let Some(last_slash) = before.rfind('/') {
                let project = before[last_slash + 1..].to_string();
                if !project.is_empty() && project != "bin" && project != "src" {
                    return Some(project);
                }
            }
        }

        // Otherwise look for common project path patterns
        for pattern in &["/sandbox/", "/projects/", "/work/", "/dev/", "/Users/"] {
            if let Some(idx) = cmd.find(pattern) {
                let after = &cmd[idx + pattern.len()..];
                // Skip past intermediate directories and find the actual project
                let parts: Vec<&str> = after.split('/').collect();
                // Get the second-to-last meaningful directory (project name)
                for (i, part) in parts.iter().enumerate() {
                    if !part.is_empty()
                        && *part != "bin"
                        && *part != "src"
                        && *part != "node_modules"
                        && i < parts.len() - 1 {
                        best_match = Some(part.to_string());
                    }
                }
            }
        }
        best_match
    };

    // System services
    if name_lower.contains("controlce") || name_lower.contains("controlcenter") {
        return "macOS Control Center".to_string();
    }
    if name_lower.contains("docker") || name_lower.contains("com.docke") {
        return match port {
            5432 => "PostgreSQL (Docker)".to_string(),
            3306 => "MySQL (Docker)".to_string(),
            6379 => "Redis (Docker)".to_string(),
            27017 => "MongoDB (Docker)".to_string(),
            _ => "Docker service".to_string(),
        };
    }

    // Node.js services
    if name_lower == "node" {
        // Check for specific frameworks
        if cmd_lower.contains("vite") {
            if let Some(project) = extract_project(&cmd_lower) {
                return format!("Vite dev server ({})", project);
            }
            return "Vite dev server".to_string();
        }
        if cmd_lower.contains("next") {
            if let Some(project) = extract_project(&cmd_lower) {
                return format!("Next.js ({})", project);
            }
            return "Next.js dev server".to_string();
        }
        if cmd_lower.contains("nx serve") || cmd_lower.contains("nx run") {
            if let Some(project) = extract_project(&cmd_lower) {
                return format!("Nx dev server ({})", project);
            }
            return "Nx dev server".to_string();
        }
        if cmd_lower.contains("webpack") {
            return "Webpack dev server".to_string();
        }
        if cmd_lower.contains("express") || cmd_lower.contains("fastify") {
            if let Some(project) = extract_project(&cmd_lower) {
                return format!("Node API ({})", project);
            }
            return "Node.js API server".to_string();
        }
        if cmd_lower.contains("react-scripts") {
            return "Create React App".to_string();
        }
        // Generic node
        if let Some(project) = extract_project(&cmd_lower) {
            return format!("Node.js ({})", project);
        }
        return "Node.js app".to_string();
    }

    // Python services
    if name_lower.contains("python") {
        if cmd_lower.contains("uvicorn") {
            return "FastAPI/ASGI server".to_string();
        }
        if cmd_lower.contains("flask") {
            return "Flask dev server".to_string();
        }
        if cmd_lower.contains("django") {
            return "Django dev server".to_string();
        }
        if let Some(project) = extract_project(&cmd_lower) {
            return format!("Python app ({})", project);
        }
        return "Python app".to_string();
    }

    // Databases
    if name_lower.contains("postgres") {
        return "PostgreSQL".to_string();
    }
    if name_lower.contains("mysql") {
        return "MySQL".to_string();
    }
    if name_lower.contains("redis") {
        return "Redis".to_string();
    }
    if name_lower.contains("mongo") {
        return "MongoDB".to_string();
    }

    // Port-based fallbacks
    match port {
        3000..=3003 => "Frontend dev server".to_string(),
        4200 => "Nx frontend".to_string(),
        5000..=5001 => "Backend API".to_string(),
        5173 => "Vite dev server".to_string(),
        8000 | 8080 | 8081 => "HTTP server".to_string(),
        5432 => "PostgreSQL".to_string(),
        3306 => "MySQL".to_string(),
        6379 => "Redis".to_string(),
        27017 => "MongoDB".to_string(),
        9229 => "Node.js debugger".to_string(),
        _ => format!("Service on port {}", port),
    }
}

fn get_process_command_full(pid: i32) -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    if let Some(process) = sys.process(sysinfo::Pid::from(pid as usize)) {
        let cmd_parts: Vec<String> = process
            .cmd()
            .iter()
            .filter_map(|s| s.to_str())
            .map(|s| s.to_string())
            .collect();
        cmd_parts.join(" ")
    } else {
        String::from("Unknown")
    }
}

fn get_process_command(pid: i32) -> String {
    let cmd = get_process_command_full(pid);
    if cmd.len() > 100 {
        let truncated: String = cmd.chars().take(97).collect();
        format!("{}...", truncated)
    } else {
        cmd
    }
}

fn kill_process(pid: i32) -> Result<()> {
    unsafe {
        if libc::kill(pid, libc::SIGTERM) == 0 {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to send SIGTERM"))
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Prepare ports list
    let mut ports = DEFAULT_PORTS.to_vec();
    ports.extend(&cli.ports);
    ports.sort();
    ports.dedup();

    // Handle --list flag
    if cli.list {
        println!("Monitored ports:");
        println!("{}", "─".repeat(50));
        for port in ports {
            let service = match port {
                3000..=3003 => "React/Next.js/Vite dev server",
                4200 => "Nx frontend (Angular/React/Vue)",
                5000..=5001 => "Flask/general backend",
                5173 => "Vite dev server",
                8000 | 8080..=8081 => "Python/Node backend",
                5432 => "PostgreSQL",
                3306 => "MySQL",
                6379 => "Redis",
                27017 => "MongoDB",
                9229 => "Node.js debugger",
                _ => "Custom port",
            };
            println!("{:>6}  {}", port, service);
        }
        return Ok(());
    }

    // Handle --scan flag (debug mode)
    if cli.scan {
        println!("Scanning {} ports...", ports.len());
        println!("{}", "─".repeat(70));
        let processes = scan_ports_debug(&ports, true);
        if processes.is_empty() {
            println!("\nNo processes found on monitored ports.");
        } else {
            println!("\nFound {} process(es):\n", processes.len());
            for p in processes {
                println!("Port: {:>5}", p.port);
                println!("  Description: {}", p.description);
                println!("  PID:         {}", p.pid);
                println!("  Process:     {}", p.name);
                println!("  Command:     {}", p.command_full);
                println!();
            }
        }
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(cli.ports);
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('r') => {
                        app.refresh();
                        app.message = Some("Refreshed".to_string());
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char('d') | KeyCode::Delete => app.kill_selected(),
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    // Dynamic footer height: taller when showing full command
    let footer_height = if app.list_state.selected().is_some() && !app.processes.is_empty() {
        5  // Show command + controls
    } else {
        3  // Just controls
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(footer_height),
        ])
        .split(f.area());

    // Header with some flair
    let header_text = if app.processes.is_empty() {
        "🔍 DevPort - No services detected"
    } else {
        "🚢 DevPort - Development Port Manager"
    };

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            header_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .block(Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(header, chunks[0]);

    // Process list with emojis and colors
    let items: Vec<ListItem> = app
        .processes
        .iter()
        .map(|p| {
            let emoji = get_service_emoji(&p.description);

            // Color code by port type
            let port_color = match p.port {
                3000..=4200 => Color::Green,     // Frontend
                5000..=5173 => Color::Yellow,    // Backend
                5432 | 3306 | 6379 | 27017 => Color::Magenta, // Databases
                _ => Color::White,
            };

            // First line: emoji, port, description, PID
            let line1 = Line::from(vec![
                Span::raw(emoji),
                Span::raw(" "),
                Span::styled(
                    format!(":{:<5}", p.port),
                    Style::default().fg(port_color).add_modifier(Modifier::BOLD)
                ),
                Span::raw(format!(" {:<35}", p.description)),
                Span::styled(
                    format!("PID {}", p.pid),
                    Style::default().fg(Color::DarkGray)
                ),
            ]);

            // Second line: indented command path
            let line2 = Line::from(Span::styled(
                format!("        → {}", p.command),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
            ));

            ListItem::new(vec![line1, line2])
        })
        .collect();

    let list_title = if app.processes.is_empty() {
        " 📭 No Processes Found ".to_string()
    } else {
        format!(" 🎯 Active Services ({}) ", app.processes.len())
    };

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(list_title)
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Footer - always show controls, add command info when selected
    let controls_line = Line::from(vec![
        Span::raw("↑/↓ or j/k: Navigate | "),
        Span::styled("d", Style::default().fg(Color::Red)),
        Span::raw(": Kill | "),
        Span::styled("r", Style::default().fg(Color::Green)),
        Span::raw(": Refresh | "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(": Quit"),
    ]);

    let footer_text = if let Some(msg) = &app.message {
        vec![
            Line::from(Span::styled(msg, Style::default().fg(Color::Yellow))),
            Line::from(""),
            controls_line,
        ]
    } else if let Some(selected) = app.list_state.selected() {
        if let Some(process) = app.processes.get(selected) {
            vec![
                Line::from(vec![
                    Span::styled("📋 Full command: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(&process.command_full, Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                controls_line,
            ]
        } else {
            vec![controls_line]
        }
    } else {
        vec![controls_line]
    };

    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(footer, chunks[2]);

    // Clear message after displaying
    if app.message.is_some() {
        app.message = None;
    }
}
