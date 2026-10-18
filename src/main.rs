mod github;
mod profile;
mod roast;
mod server;

use clap::{Parser, Subcommand};
use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// Judges a GitHub profile. Harshly.
#[derive(Parser)]
#[command(version, args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// GitHub username to roast
    #[arg(required = true)]
    username: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve roasts over HTTP, with health checks and Prometheus metrics
    Serve {
        /// Port to listen on
        #[arg(long, env = "PORT", default_value_t = 8080)]
        port: u16,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Some(Command::Serve { port }) = cli.command {
        return match server::run(port) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        };
    }

    let username = cli.username.expect("clap requires a username");
    if !is_valid_username(&username) {
        eprintln!("error: '{username}' is not a valid GitHub username");
        return ExitCode::from(2);
    }

    let client = github::Client::from_env();
    let fetched = client
        .user(&username)
        .and_then(|user| Ok((user, client.repos(&username)?)));
    let (user, repos) = match fetched {
        Ok(data) => data,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let profile = profile::Profile {
        user,
        repos,
        today: today(),
    };
    print_verdict(&profile, &roast::judge(&profile));
    ExitCode::SUCCESS
}

/// GitHub usernames: 1-39 letters, digits or single hyphens, not at either end.
fn is_valid_username(name: &str) -> bool {
    (1..=39).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

fn today() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    (secs / 86_400) as i64
}

fn print_verdict(profile: &profile::Profile, verdict: &roast::Verdict) {
    let color = std::io::stdout().is_terminal();
    let paint = |code: &str, text: &str| {
        if color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };

    let user = &profile.user;
    let who = match &user.name {
        Some(name) if !name.is_empty() => format!("{name} (@{})", user.login),
        _ => format!("@{}", user.login),
    };
    println!("\n{} {}\n", paint("1", "Roasting"), paint("1;36", &who));

    if verdict.burns.is_empty() {
        println!(
            "  {}",
            paint("32", "Nothing to roast. Suspicious. What are you hiding?")
        );
    }
    for burn in &verdict.burns {
        println!("  {} {}", paint("31", "🔥"), burn.text);
    }

    let filled = verdict.score as usize;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
    let score_color = match verdict.score {
        8..=10 => "32",
        5..=7 => "33",
        _ => "31",
    };
    println!(
        "\n  Score: {} {}/10",
        paint(score_color, &bar),
        paint("1", &verdict.score.to_string())
    );
    println!("  {}\n", paint("3", verdict.summary));
}

#[cfg(test)]
mod tests {
    use super::is_valid_username;

    #[test]
    fn accepts_real_usernames() {
        for name in ["torvalds", "ankur-kalita", "a", "x1-y2"] {
            assert!(is_valid_username(name), "{name}");
        }
    }

    #[test]
    fn rejects_bad_usernames() {
        for name in [
            "",
            "-lead",
            "trail-",
            "dou--ble",
            "../etc",
            "with space",
            &"a".repeat(40),
        ] {
            assert!(!is_valid_username(name), "{name}");
        }
    }
}
