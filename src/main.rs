use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
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
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
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

    /// Auto-refresh interval in seconds (0 to disable, default: 2)
    #[arg(long, default_value = "2")]
    refresh: u64,
}

enum AppEvent {
    Key(KeyEvent),
    Tick,
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
    ppid: Option<i32>,
    name: String,
    command: String,
    command_full: String,
    description: String,
    project: Option<String>,
}

struct ProcessGroup {
    name: String,
    process_indices: Vec<usize>,
}

enum DisplayRow {
    GroupHeader { name: String, count: usize },
    Process { process_index: usize },
}

struct App {
    processes: Vec<PortProcess>,
    display_rows: Vec<DisplayRow>,
    list_state: ListState,
    message: Option<String>,
    ports: Vec<u16>,
    auto_refresh: bool,
    refresh_interval: Duration,
}

impl App {
    fn new(custom_ports: Vec<u16>, refresh_secs: u64) -> Self {
        let mut ports = DEFAULT_PORTS.to_vec();
        ports.extend(custom_ports);
        ports.sort();
        ports.dedup();

        let mut app = App {
            processes: Vec::new(),
            display_rows: Vec::new(),
            list_state: ListState::default(),
            message: None,
            ports,
            auto_refresh: refresh_secs > 0,
            refresh_interval: Duration::from_secs(if refresh_secs > 0 { refresh_secs } else { 2 }),
        };
        app.refresh();
        app.select_first_process();
        app
    }

    fn rebuild_display_rows(&mut self) {
        let groups = compute_groups(&self.processes);
        self.display_rows.clear();
        for group in &groups {
            if group.process_indices.is_empty() {
                continue;
            }
            self.display_rows.push(DisplayRow::GroupHeader {
                name: group.name.clone(),
                count: group.process_indices.len(),
            });
            for &idx in &group.process_indices {
                self.display_rows.push(DisplayRow::Process { process_index: idx });
            }
        }
    }

    fn select_first_process(&mut self) {
        for (i, row) in self.display_rows.iter().enumerate() {
            if matches!(row, DisplayRow::Process { .. }) {
                self.list_state.select(Some(i));
                return;
            }
        }
        self.list_state.select(None);
    }

    fn selected_process(&self) -> Option<&PortProcess> {
        self.list_state.selected().and_then(|i| {
            match self.display_rows.get(i) {
                Some(DisplayRow::Process { process_index }) => self.processes.get(*process_index),
                _ => None,
            }
        })
    }

    fn refresh(&mut self) {
        self.processes = scan_ports(&self.ports);
        self.rebuild_display_rows();
        if self.processes.is_empty() {
            self.list_state.select(None);
        } else if let Some(selected) = self.list_state.selected() {
            if selected >= self.display_rows.len() {
                self.select_first_process();
            }
        }
    }

    fn next(&mut self) {
        if self.display_rows.is_empty() {
            return;
        }
        let start = self.list_state.selected().map(|s| s + 1).unwrap_or(0);
        // Search forward from current position
        for i in start..self.display_rows.len() {
            if matches!(self.display_rows[i], DisplayRow::Process { .. }) {
                self.list_state.select(Some(i));
                return;
            }
        }
        // Wrap to beginning
        for i in 0..self.display_rows.len() {
            if matches!(self.display_rows[i], DisplayRow::Process { .. }) {
                self.list_state.select(Some(i));
                return;
            }
        }
    }

    fn previous(&mut self) {
        if self.display_rows.is_empty() {
            return;
        }
        let start = self.list_state.selected().unwrap_or(0);
        // Search backward from current position
        if start > 0 {
            for i in (0..start).rev() {
                if matches!(self.display_rows[i], DisplayRow::Process { .. }) {
                    self.list_state.select(Some(i));
                    return;
                }
            }
        }
        // Wrap to end
        for i in (0..self.display_rows.len()).rev() {
            if matches!(self.display_rows[i], DisplayRow::Process { .. }) {
                self.list_state.select(Some(i));
                return;
            }
        }
    }

    fn refresh_preserving_selection(&mut self) {
        let selected_pid = self.selected_process().map(|p| p.pid);
        self.processes = scan_ports(&self.ports);
        self.rebuild_display_rows();
        if let Some(pid) = selected_pid {
            // Find the display row that points to the process with this PID
            for (i, row) in self.display_rows.iter().enumerate() {
                if let DisplayRow::Process { process_index } = row {
                    if self.processes.get(*process_index).map(|p| p.pid) == Some(pid) {
                        self.list_state.select(Some(i));
                        return;
                    }
                }
            }
        }
        if self.processes.is_empty() {
            self.list_state.select(None);
        } else if let Some(sel) = self.list_state.selected() {
            if sel >= self.display_rows.len() {
                self.select_first_process();
            }
        }
    }

    fn kill_selected(&mut self) {
        if let Some(process) = self.selected_process() {
            let pid = process.pid;
            match kill_process(pid) {
                Ok(_) => {
                    self.message = Some(format!("Killed PID {} successfully", pid));
                    self.refresh();
                }
                Err(e) => {
                    self.message = Some(format!("Failed to kill PID {}: {}", pid, e));
                }
            }
        }
    }
}

fn compute_groups(processes: &[PortProcess]) -> Vec<ProcessGroup> {
    use std::collections::HashMap;

    let mut groups: Vec<ProcessGroup> = Vec::new();
    let mut grouped: Vec<bool> = vec![false; processes.len()];

    // Pass 1: group by project name
    let mut project_map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, p) in processes.iter().enumerate() {
        if let Some(ref project) = p.project {
            project_map.entry(project.clone()).or_default().push(i);
        }
    }
    let mut project_names: Vec<String> = project_map.keys().cloned().collect();
    project_names.sort();
    for name in project_names {
        let indices = project_map.remove(&name).unwrap();
        if indices.len() >= 1 {
            for &i in &indices {
                grouped[i] = true;
            }
            groups.push(ProcessGroup {
                name,
                process_indices: indices,
            });
        }
    }

    // Pass 2: among ungrouped, group by shared PPID
    let mut ppid_map: HashMap<i32, Vec<usize>> = HashMap::new();
    for (i, p) in processes.iter().enumerate() {
        if !grouped[i] {
            if let Some(ppid) = p.ppid {
                ppid_map.entry(ppid).or_default().push(i);
            }
        }
    }
    let mut ppids: Vec<i32> = ppid_map.keys().cloned().collect();
    ppids.sort();
    for ppid in ppids {
        let indices = ppid_map.remove(&ppid).unwrap();
        if indices.len() >= 2 {
            for &i in &indices {
                grouped[i] = true;
            }
            groups.push(ProcessGroup {
                name: format!("PID group (PPID {})", ppid),
                process_indices: indices,
            });
        }
    }

    // Pass 3: remaining ungrouped go to "Other"
    let other: Vec<usize> = (0..processes.len()).filter(|i| !grouped[*i]).collect();
    if !other.is_empty() {
        groups.push(ProcessGroup {
            name: "Other".to_string(),
            process_indices: other,
        });
    }

    groups
}

/// Get process info from a pre-loaded System instance.
/// Returns (command_full, command_truncated, parent_pid).
fn get_process_info(sys: &System, pid: i32) -> (String, String, Option<i32>) {
    if let Some(process) = sys.process(sysinfo::Pid::from(pid as usize)) {
        let cmd_parts: Vec<String> = process
            .cmd()
            .iter()
            .filter_map(|s| s.to_str())
            .map(|s| s.to_string())
            .collect();
        let command_full = cmd_parts.join(" ");
        let command = if command_full.chars().count() > 100 {
            let truncated: String = command_full.chars().take(97).collect();
            format!("{}...", truncated)
        } else {
            command_full.clone()
        };
        let ppid = process.parent().map(|p| p.as_u32() as i32);
        (command_full, command, ppid)
    } else {
        (String::from("Unknown"), String::from("Unknown"), None)
    }
}

fn scan_ports(ports: &[u16]) -> Vec<PortProcess> {
    scan_ports_debug(ports, false)
}

fn scan_ports_debug(ports: &[u16], debug: bool) -> Vec<PortProcess> {
    let mut results = Vec::new();
    let mut sys = System::new_all();
    sys.refresh_all();

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
                                let (command_full, command, ppid) = get_process_info(&sys, pid);
                                let description = infer_description(port, &name, &command_full);
                                let project = extract_project(&command_full.to_lowercase());
                                if debug {
                                    eprintln!("  Found: {} PID {} - {}", name, pid, description);
                                }
                                results.push(PortProcess {
                                    port,
                                    pid,
                                    ppid,
                                    name,
                                    command,
                                    command_full,
                                    description,
                                    project,
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

fn extract_project(cmd: &str) -> Option<String> {
    let mut best_match: Option<String> = None;

    // First try to find node_modules or similar markers
    if let Some(idx) = cmd.find("/node_modules/") {
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
            let parts: Vec<&str> = after.split('/').collect();
            for (i, part) in parts.iter().enumerate() {
                if !part.is_empty()
                    && *part != "bin"
                    && *part != "src"
                    && *part != "node_modules"
                    && i < parts.len() - 1
                {
                    best_match = Some(part.to_string());
                }
            }
        }
    }
    best_match
}

fn infer_description(port: u16, process_name: &str, command: &str) -> String {
    let cmd_lower = command.to_lowercase();
    let name_lower = process_name.to_lowercase();

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
    let mut app = App::new(cli.ports, cli.refresh);
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
    let (tx, rx) = mpsc::channel::<AppEvent>();

    // Key event thread
    let tx_key = tx.clone();
    thread::spawn(move || loop {
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if tx_key.send(AppEvent::Key(key)).is_err() {
                    break;
                }
            }
        }
    });

    // Tick thread
    let tx_tick = tx;
    let tick_interval = app.refresh_interval;
    thread::spawn(move || loop {
        thread::sleep(tick_interval);
        if tx_tick.send(AppEvent::Tick).is_err() {
            break;
        }
    });

    loop {
        terminal.draw(|f| ui(f, app))?;

        match rx.recv() {
            Ok(AppEvent::Key(key)) => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('r') => {
                    app.refresh();
                    app.message = Some("Refreshed".to_string());
                }
                KeyCode::Char('a') => {
                    app.auto_refresh = !app.auto_refresh;
                    app.message = Some(if app.auto_refresh {
                        format!("Auto-refresh enabled ({}s)", app.refresh_interval.as_secs())
                    } else {
                        "Auto-refresh disabled".to_string()
                    });
                }
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Char('d') | KeyCode::Delete => app.kill_selected(),
                _ => {}
            },
            Ok(AppEvent::Tick) => {
                if app.auto_refresh {
                    app.refresh_preserving_selection();
                }
            }
            Err(_) => return Ok(()),
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    // Dynamic footer height: taller when showing full command
    let footer_height = if app.selected_process().is_some() {
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
    let refresh_status = if app.auto_refresh {
        format!(" [Auto-refresh: {}s]", app.refresh_interval.as_secs())
    } else {
        " [Auto-refresh: off]".to_string()
    };
    let header_text = if app.processes.is_empty() {
        format!("🔍 DevPort - No services detected{}", refresh_status)
    } else {
        format!("🚢 DevPort - Development Port Manager{}", refresh_status)
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

    // Process list with emojis, colors, and group headers
    let items: Vec<ListItem> = app
        .display_rows
        .iter()
        .map(|row| match row {
            DisplayRow::GroupHeader { name, count } => {
                ListItem::new(vec![Line::from(vec![
                    Span::styled(
                        format!("── {} ({}) ", name, count),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "─".repeat(30),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])])
            }
            DisplayRow::Process { process_index } => {
                let p = &app.processes[*process_index];
                let emoji = get_service_emoji(&p.description);

                let port_color = match p.port {
                    3000..=4200 => Color::Green,
                    5000..=5173 => Color::Yellow,
                    5432 | 3306 | 6379 | 27017 => Color::Magenta,
                    _ => Color::White,
                };

                let line1 = Line::from(vec![
                    Span::raw(emoji),
                    Span::raw(" "),
                    Span::styled(
                        format!(":{:<5}", p.port),
                        Style::default()
                            .fg(port_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" {:<35}", p.description)),
                    Span::styled(
                        format!("PID {}", p.pid),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);

                let line2 = Line::from(Span::styled(
                    format!("        → {}", p.command),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ));

                ListItem::new(vec![line1, line2])
            }
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
        Span::styled("a", Style::default().fg(Color::Blue)),
        Span::raw(": Auto-refresh | "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(": Quit"),
    ]);

    let footer_text = if let Some(msg) = &app.message {
        vec![
            Line::from(Span::styled(msg, Style::default().fg(Color::Yellow))),
            Line::from(""),
            controls_line,
        ]
    } else if let Some(process) = app.selected_process() {
        let avail = (f.area().width as usize).saturating_sub(20);
        let cmd_display = if process.command_full.chars().count() > avail {
            let truncated: String = process.command_full.chars().take(avail.saturating_sub(3)).collect();
            format!("{}...", truncated)
        } else {
            process.command_full.clone()
        };
        vec![
            Line::from(vec![
                Span::styled("📋 Full command: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(cmd_display, Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            controls_line,
        ]
    } else {
        vec![controls_line]
    };

    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Clear message after displaying
    if app.message.is_some() {
        app.message = None;
    }
}
