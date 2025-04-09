use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use ansi_term::Colour::{Blue, Green, Yellow};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use dirs::home_dir;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use rusqlite::{params, Connection, Result};
use shellexpand::tilde;

#[derive(Parser)]
#[command(name = "pathranger")]
#[command(about = "A file system navigation enhancement tool", long_about = None)]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Mark current directory with a tag
    Mark {
        /// Tag name
        tag: String,
    },
    
    /// Jump to a tagged directory
    Goto {
        /// Tag name
        tag: String,
    },
    
    /// Add current directory to tracked paths
    Add,
    
    /// List your most visited directories
    Top {
        /// Number of directories to show
        #[arg(short, long, default_value_t = 10)]
        count: usize,
    },
    
    /// Show recently visited directories
    Recent {
        /// Number of directories to show
        #[arg(short, long, default_value_t = 10)]
        count: usize,
    },
    
    /// Search across your visited directories
    Search {
        /// Text to search for
        query: String,
    },
    
    /// List all tags
    Tags,
    
    /// Remove a tag
    Untag {
        /// Tag to remove
        tag: String,
    },
    
    /// Record a visit to a directory (usually called from shell integration)
    Record {
        /// Directory path
        path: String,
    },
    
    /// Generate shell integration code
    Init {
        /// Shell type (bash, zsh, fish)
        #[arg(short, long, default_value = "bash")]
        shell: String,
    },
}

fn setup_database() -> Result<Connection> {
    let data_dir = match dirs::data_dir() {
        Some(dir) => dir.join("pathranger"),
        None => {
            eprintln!("Could not determine data directory");
            process::exit(1);
        }
    };
    
    fs::create_dir_all(&data_dir).map_err(|e| {
        eprintln!("Could not create data directory: {}", e);
        process::exit(1);
    }).unwrap();
    
    let db_path = data_dir.join("pathranger.db");
    let conn = Connection::open(&db_path)?;
    
    // Create tables if they don't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS directories (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            visit_count INTEGER NOT NULL DEFAULT 1,
            last_visited DATETIME NOT NULL
        )",
        [],
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            path TEXT NOT NULL,
            created_at DATETIME NOT NULL
        )",
        [],
    )?;
    
    Ok(conn)
}

fn record_visit(conn: &Connection, path: &str) -> Result<()> {
    let expanded_path = tilde(path).into_owned();
    
    // Check if the directory exists
    if !Path::new(&expanded_path).is_dir() {
        eprintln!("Directory does not exist: {}", expanded_path);
        return Ok(());
    }
    
    // Try to update existing entry
    let now = Local::now().to_rfc3339();
    let rows_affected = conn.execute(
        "UPDATE directories SET visit_count = visit_count + 1, last_visited = ?1 WHERE path = ?2",
        params![now, expanded_path],
    )?;
    
    // If no rows were affected, insert a new entry
    if rows_affected == 0 {
        conn.execute(
            "INSERT INTO directories (path, visit_count, last_visited) VALUES (?1, 1, ?2)",
            params![expanded_path, now],
        )?;
    }
    
    Ok(())
}

fn mark_directory(conn: &Connection, tag: &str, path: Option<&str>) -> Result<()> {
    let path = match path {
        Some(p) => shellexpand::tilde(p).into_owned(),
        None => std::env::current_dir()
            .map_err(|e| {
                eprintln!("Could not get current directory: {}", e);
                process::exit(1);
            })
            .unwrap()
            .to_string_lossy()
            .to_string(),
    };
    
    // Check if the directory exists
    if !Path::new(&path).is_dir() {
        eprintln!("Directory does not exist: {}", path);
        process::exit(1);
    }
    
    // Check if tag already exists
    let mut stmt = conn.prepare("SELECT id FROM tags WHERE name = ?1")?;
    let exists = stmt.exists(params![tag])?;
    
    let now = Local::now().to_rfc3339();
    if exists {
        // Update existing tag
        conn.execute(
            "UPDATE tags SET path = ?1, created_at = ?2 WHERE name = ?3",
            params![path, now, tag],
        )?;
        println!("Updated tag '{}' to point to '{}'", Green.bold().paint(tag), Blue.paint(&path));
    } else {
        // Create new tag
        conn.execute(
            "INSERT INTO tags (name, path, created_at) VALUES (?1, ?2, ?3)",
            params![tag, path, now],
        )?;
        println!("Created tag '{}' for '{}'", Green.bold().paint(tag), Blue.paint(&path));
    }
    
    // Also record a visit
    record_visit(conn, &path)?;
    
    Ok(())
}

fn goto_tag(conn: &Connection, tag: &str) -> Result<()> {
    let mut stmt = conn.prepare("SELECT path FROM tags WHERE name = ?1")?;
    let path: Result<String, rusqlite::Error> = stmt.query_row(params![tag], |row| row.get(0));
    
    match path {
        Ok(path) => {
            // Print the path for the shell wrapper to cd into
            println!("{}", path);
            record_visit(conn, &path)?;
        }
        Err(_) => {
            eprintln!("Tag '{}' not found", tag);
            process::exit(1);
        }
    }
    
    Ok(())
}

fn list_top_directories(conn: &Connection, count: usize) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT path, visit_count, last_visited FROM directories 
         ORDER BY visit_count DESC LIMIT ?1",
    )?;
    
    let paths = stmt.query_map(params![count], |row| {
        let path: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        let last_visited: String = row.get(2)?;
        
        // Parse the date string
        let last_visited_date = DateTime::parse_from_rfc3339(&last_visited)
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .with_timezone(&Local);
        
        Ok((path, count, last_visited_date))
    })?;
    
    println!("Your most frequently visited directories:");
    println!("{:<4} {:<8} {:<20} {}", "", "VISITS", "LAST VISITED", "PATH");
    
    for (i, path_result) in paths.enumerate() {
        match path_result {
            Ok((path, count, last_visited)) => {
                println!(
                    "{:<4} {:<8} {:<20} {}",
                    i + 1,
                    Yellow.paint(count.to_string()),
                    last_visited.format("%Y-%m-%d %H:%M"),
                    Blue.paint(format_path(&path))
                );
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}

fn list_recent_directories(conn: &Connection, count: usize) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT path, visit_count, last_visited FROM directories 
         ORDER BY last_visited DESC LIMIT ?1",
    )?;
    
    let paths = stmt.query_map(params![count], |row| {
        let path: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        let last_visited: String = row.get(2)?;
        
        // Parse the date string
        let last_visited_date = DateTime::parse_from_rfc3339(&last_visited)
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .with_timezone(&Local);
        
        Ok((path, count, last_visited_date))
    })?;
    
    println!("Your recently visited directories:");
    println!("{:<4} {:<8} {:<20} {}", "", "VISITS", "LAST VISITED", "PATH");
    
    for (i, path_result) in paths.enumerate() {
        match path_result {
            Ok((path, count, last_visited)) => {
                println!(
                    "{:<4} {:<8} {:<20} {}",
                    i + 1,
                    Yellow.paint(count.to_string()),
                    last_visited.format("%Y-%m-%d %H:%M"),
                    Blue.paint(format_path(&path))
                );
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}

fn search_directories(conn: &Connection, query: &str) -> Result<()> {
    let mut stmt = conn.prepare("SELECT path FROM directories")?;
    let paths = stmt.query_map([], |row| {
        let path: String = row.get(0)?;
        Ok(path)
    })?;
    
    let matcher = SkimMatcherV2::default();
    let mut matches = Vec::new();
    
    for path_result in paths {
        match path_result {
            Ok(path) => {
                if let Some(score) = matcher.fuzzy_match(&path, query) {
                    matches.push((path, score));
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    // Sort by score (descending)
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    
    if matches.is_empty() {
        println!("No matching directories found for '{}'", query);
        return Ok(());
    }
    
    println!("Search results for '{}':", query);
    println!("{:<4} {:<8} {}", "", "SCORE", "PATH");
    
    for (i, (path, score)) in matches.iter().enumerate().take(10) {
        println!(
            "{:<4} {:<8} {}",
            i + 1,
            Yellow.paint(score.to_string()),
            Blue.paint(format_path(path))
        );
    }
    
    Ok(())
}

fn list_tags(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT name, path FROM tags ORDER BY name")?;
    let tag_rows = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let path: String = row.get(1)?;
        Ok((name, path))
    })?;
    
    println!("Your tags:");
    println!("{:<20} {}", "TAG", "PATH");
    
    for tag_result in tag_rows {
        match tag_result {
            Ok((name, path)) => {
                println!(
                    "{:<20} {}",
                    Green.bold().paint(name),
                    Blue.paint(format_path(&path))
                );
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}

fn remove_tag(conn: &Connection, tag: &str) -> Result<()> {
    let rows_affected = conn.execute("DELETE FROM tags WHERE name = ?1", params![tag])?;
    
    if rows_affected > 0 {
        println!("Tag '{}' removed", tag);
    } else {
        println!("Tag '{}' not found", tag);
    }
    
    Ok(())
}

fn add_current_directory(conn: &Connection) -> Result<()> {
    let current_dir = std::env::current_dir()
        .map_err(|e| {
            eprintln!("Could not get current directory: {}", e);
            process::exit(1);
        })
        .unwrap();
    
    record_visit(conn, &current_dir.to_string_lossy())?;
    println!("Added '{}' to tracked directories", Blue.paint(format_path(&current_dir.to_string_lossy())));
    
    Ok(())
}

fn generate_shell_init(shell: &str) -> Result<()> {
    match shell {
        "bash" => {
            println!("# Add this to your ~/.bashrc");
            println!("eval \"$(pathranger init --shell bash)\"");
            println!("");
            println!("# PathRanger shell integration for bash");
            println!("__pathranger_cd() {{");
            println!("    local dir=\"$1\"");
            println!("    if [ -d \"$dir\" ]; then");
            println!("        cd \"$dir\" || return");
            println!("        pathranger record \"$PWD\" >/dev/null 2>&1");
            println!("    fi");
            println!("}}");
            println!("");
            println!("# Override cd");
            println!("cd() {{");
            println!("    __pathranger_cd \"$@\"");
            println!("}}");
            println!("");
            println!("# Record initial directory");
            println!("pathranger record \"$PWD\" >/dev/null 2>&1");
            println!("");
            println!("# pr goto alias");
            println!("pr() {{");
            println!("    if [ \"$1\" = \"goto\" ] && [ -n \"$2\" ]; then");
            println!("        local dir");
            println!("        dir=$(pathranger goto \"$2\")");
            println!("        if [ -n \"$dir\" ]; then");
            println!("            __pathranger_cd \"$dir\"");
            println!("        fi");
            println!("    else");
            println!("        pathranger \"$@\"");
            println!("    fi");
            println!("}}");
        }
        "zsh" => {
            println!("# Add this to your ~/.zshrc");
            println!("eval \"$(pathranger init --shell zsh)\"");
            println!("");
            println!("# PathRanger shell integration for zsh");
            println!("__pathranger_cd() {{");
            println!("    local dir=\"$1\"");
            println!("    if [[ -d \"$dir\" ]]; then");
            println!("        builtin cd \"$dir\" || return");
            println!("        pathranger record \"$PWD\" >/dev/null 2>&1");
            println!("    fi");
            println!("}}");
            println!("");
            println!("# Override cd");
            println!("cd() {{");
            println!("    __pathranger_cd \"$@\"");
            println!("}}");
            println!("");
            println!("# Record initial directory");
            println!("pathranger record \"$PWD\" >/dev/null 2>&1");
            println!("");
            println!("# pr goto alias");
            println!("pr() {{");
            println!("    if [[ \"$1\" = \"goto\" && -n \"$2\" ]]; then");
            println!("        local dir");
            println!("        dir=$(pathranger goto \"$2\")");
            println!("        if [[ -n \"$dir\" ]]; then");
            println!("            __pathranger_cd \"$dir\"");
            println!("        fi");
            println!("    else");
            println!("        pathranger \"$@\"");
            println!("    fi");
            println!("}}");
        }
        "fish" => {
            println!("# Add this to your ~/.config/fish/config.fish");
            println!("eval (pathranger init --shell fish)");
            println!("");
            println!("# PathRanger shell integration for fish");
            println!("function __pathranger_cd");
            println!("    set dir $argv[1]");
            println!("    if test -d \"$dir\"");
            println!("        builtin cd \"$dir\"");
            println!("        pathranger record \"$PWD\" >/dev/null 2>&1");
            println!("    end");
            println!("end");
            println!("");
            println!("# Override cd");
            println!("function cd");
            println!("    __pathranger_cd $argv");
            println!("end");
            println!("");
            println!("# Record initial directory");
            println!("pathranger record \"$PWD\" >/dev/null 2>&1");
            println!("");
            println!("# pr goto alias");
            println!("function pr");
            println!("    if test \"$argv[1]\" = \"goto\"; and test -n \"$argv[2]\"");
            println!("        set dir (pathranger goto \"$argv[2]\")");
            println!("        if test -n \"$dir\"");
            println!("            __pathranger_cd \"$dir\"");
            println!("        end");
            println!("    else");
            println!("        pathranger $argv");
            println!("    end");
            println!("end");
        }
        _ => {
            eprintln!("Unsupported shell: {}", shell);
            eprintln!("Supported shells: bash, zsh, fish");
            process::exit(1);
        }
    }
    
    Ok(())
}

fn format_path(path: &str) -> String {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let home_str = home.to_string_lossy();
    
    if path.starts_with(&*home_str) {
        let relative_path = path.strip_prefix(&*home_str).unwrap_or(path);
        format!("~{}", relative_path)
    } else {
        path.to_string()
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = setup_database()?;
    
    match cli.command {
        Some(Commands::Mark { tag }) => mark_directory(&conn, &tag, None)?,
        Some(Commands::Goto { tag }) => goto_tag(&conn, &tag)?,
        Some(Commands::Add) => add_current_directory(&conn)?,
        Some(Commands::Top { count }) => list_top_directories(&conn, count)?,
        Some(Commands::Recent { count }) => list_recent_directories(&conn, count)?,
        Some(Commands::Search { query }) => search_directories(&conn, &query)?,
        Some(Commands::Tags) => list_tags(&conn)?,
        Some(Commands::Untag { tag }) => remove_tag(&conn, &tag)?,
        Some(Commands::Record { path }) => record_visit(&conn, &path)?,
        Some(Commands::Init { shell }) => generate_shell_init(&shell)?,
        None => {
            eprintln!("No command specified");
            eprintln!("Try 'pathranger --help' for more information");
        }
    }
    
    Ok(())
}