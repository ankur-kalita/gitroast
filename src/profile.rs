//! The data gitroast judges. Pure: no network, no clock.

use serde::Deserialize;

/// A GitHub user, as returned by `GET /users/{name}`.
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub login: String,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub followers: u32,
    pub following: u32,
    pub created_at: String,
}

/// One public repo, as returned by `GET /users/{name}/repos`.
#[derive(Debug, Clone, Deserialize)]
pub struct Repo {
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub fork: bool,
    pub stargazers_count: u32,
    pub pushed_at: Option<String>,
    /// Kilobytes; 0 means nothing was ever pushed.
    #[serde(default)]
    pub size: u32,
}

/// Everything the roast rules need, plus "today" so the rules never read a clock.
#[derive(Debug, Clone)]
pub struct Profile {
    pub user: User,
    pub repos: Vec<Repo>,
    pub today: i64,
}

impl Profile {
    /// Repos the user actually made, not forked.
    pub fn own_repos(&self) -> impl Iterator<Item = &Repo> {
        self.repos.iter().filter(|r| !r.fork)
    }

    /// How many days ago a GitHub timestamp was, or `None` if it can't be read.
    pub fn days_since(&self, timestamp: &str) -> Option<i64> {
        days_from_timestamp(timestamp).map(|d| self.today - d)
    }
}

/// Turns "2020-01-15T10:00:00Z" into a day number (days since 1970-01-01).
pub fn days_from_timestamp(timestamp: &str) -> Option<i64> {
    let date = timestamp.get(..10)?;
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days-from-civil algorithm.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_unix_epoch_as_day_zero() {
        assert_eq!(days_from_timestamp("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn counts_leap_days() {
        let a = days_from_timestamp("2024-02-28T00:00:00Z").unwrap();
        let b = days_from_timestamp("2024-03-01T00:00:00Z").unwrap();
        assert_eq!(b - a, 2);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(days_from_timestamp("yesterday"), None);
        assert_eq!(days_from_timestamp("2024-13-01T00:00:00Z"), None);
    }

    #[test]
    fn parses_github_repo_json() {
        let json = r#"{"name":"todo-app","description":null,"language":"JavaScript",
                       "fork":false,"stargazers_count":0,"pushed_at":"2021-03-04T05:06:07Z",
                       "extra_field_we_ignore":true}"#;
        let repo: Repo = serde_json::from_str(json).unwrap();
        assert_eq!(repo.name, "todo-app");
        assert!(repo.description.is_none());
    }
}
