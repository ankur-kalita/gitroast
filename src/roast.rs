//! The judging. Pure: a `Profile` goes in, a `Verdict` comes out.

use crate::profile::{Profile, Repo};

/// One roast line, and how many points it costs out of 10.
#[derive(Debug, Clone, PartialEq)]
pub struct Burn {
    pub text: String,
    pub penalty: u32,
}

#[derive(Debug, Clone)]
pub struct Verdict {
    pub burns: Vec<Burn>,
    pub score: u32,
    pub summary: &'static str,
}

const TUTORIAL_WORDS: &[&str] = &[
    "todo", "calculator", "weather", "clone", "tic-tac-toe", "tictactoe", "portfolio",
    "landing-page", "netflix", "spotify", "amazon",
];
const LAZY_NAMES: &[&str] = &[
    "test", "test1", "test2", "new", "untitled", "project", "demo", "asdf", "temp", "sample",
    "practice", "my-app", "myapp", "hello-world",
];
const BUZZWORDS: &[&str] = &[
    "passionate", "aspiring", "enthusiast", "ninja", "rockstar", "guru", "10x", "lifelong learner",
];

type Rule = fn(&Profile) -> Option<Burn>;

const RULES: &[Rule] = &[
    ghost,
    missing_descriptions,
    tutorial_projects,
    lazy_names,
    graveyard,
    zero_stars,
    fork_collector,
    unrequited_follows,
    bio,
    one_language,
    slow_starter,
];

pub fn judge(profile: &Profile) -> Verdict {
    let burns: Vec<Burn> = RULES.iter().filter_map(|rule| rule(profile)).collect();
    let lost: u32 = burns.iter().map(|b| b.penalty).sum();
    let score = 10u32.saturating_sub(lost).max(1);
    Verdict { burns, score, summary: summary(score) }
}

fn summary(score: u32) -> &'static str {
    match score {
        9..=10 => "Annoyingly solid. Nothing much to roast.",
        7..=8 => "Decent. A few skeletons in the repo closet.",
        5..=6 => "Mid. Recruiters will scroll past politely.",
        3..=4 => "Rough. This profile needs a weekend of cleanup.",
        _ => "A crime scene. GitHub should send a welfare check.",
    }
}

fn burn(text: String, penalty: u32) -> Option<Burn> {
    Some(Burn { text, penalty })
}

/// "a, b and c" style list of up to three repo names.
fn examples<'a>(repos: impl Iterator<Item = &'a Repo>) -> String {
    let names: Vec<&str> = repos.take(3).map(|r| r.name.as_str()).collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {}", rest.join(", "), last),
    }
}

fn ghost(p: &Profile) -> Option<Burn> {
    if p.own_repos().next().is_some() {
        return None;
    }
    burn("Zero original repos. A ghost with a GitHub account.".into(), 5)
}

fn missing_descriptions(p: &Profile) -> Option<Burn> {
    let total = p.own_repos().count();
    let empty = p
        .own_repos()
        .filter(|r| r.description.as_deref().map_or(true, |d| d.trim().is_empty()))
        .count();
    if total < 3 || empty * 2 < total {
        return None;
    }
    burn(format!("{empty} of {total} repos have no description. Mysterious. Or lazy."), 1)
}

fn tutorial_projects(p: &Profile) -> Option<Burn> {
    let is_tutorial = |r: &&Repo| {
        let name = r.name.to_lowercase();
        TUTORIAL_WORDS.iter().any(|w| name.contains(w))
    };
    let count = p.own_repos().filter(is_tutorial).count();
    if count < 2 {
        return None;
    }
    let names = examples(p.own_repos().filter(is_tutorial));
    burn(format!("{count} tutorial projects ({names}). The YouTube pipeline is strong here."), 2)
}

fn lazy_names(p: &Profile) -> Option<Burn> {
    let is_lazy = |r: &&Repo| LAZY_NAMES.contains(&r.name.to_lowercase().as_str());
    if p.own_repos().filter(is_lazy).next().is_none() {
        return None;
    }
    let names = examples(p.own_repos().filter(is_lazy));
    burn(format!("Repos named {names}. Naming things is hard, apparently."), 1)
}

fn graveyard(p: &Profile) -> Option<Burn> {
    let total = p.own_repos().count();
    let dead = p
        .own_repos()
        .filter(|r| r.pushed_at.as_deref().and_then(|t| p.days_since(t)).is_some_and(|d| d > 365))
        .count();
    if total < 3 || dead * 2 < total {
        return None;
    }
    burn(format!("{dead} repos untouched for over a year. A graveyard with READMEs."), 2)
}

fn zero_stars(p: &Profile) -> Option<Burn> {
    let total = p.own_repos().count();
    let stars: u32 = p.own_repos().map(|r| r.stargazers_count).sum();
    if total < 3 || stars > 0 {
        return None;
    }
    burn(format!("{total} repos, 0 stars in total. Not even your mum."), 1)
}

fn fork_collector(p: &Profile) -> Option<Burn> {
    let forks = p.repos.iter().filter(|r| r.fork).count();
    let own = p.own_repos().count();
    if forks < 3 || forks <= own {
        return None;
    }
    burn(format!("{forks} forks vs {own} originals. A collector, not a creator."), 1)
}

fn unrequited_follows(p: &Profile) -> Option<Burn> {
    let (following, followers) = (p.user.following, p.user.followers);
    if following < 10 || following < followers.max(1) * 3 {
        return None;
    }
    burn(format!("Follows {following} people, followed by {followers}. Unrequited."), 1)
}

fn bio(p: &Profile) -> Option<Burn> {
    let Some(bio) = p.user.bio.as_deref().filter(|b| !b.trim().is_empty()) else {
        return burn("No bio. Letting the code speak. The code is quiet.".into(), 1);
    };
    let lower = bio.to_lowercase();
    let word = BUZZWORDS.iter().find(|w| lower.contains(*w))?;
    burn(format!("Bio says \"{word}\". So does every LinkedIn headline on earth."), 1)
}

fn one_language(p: &Profile) -> Option<Burn> {
    let langs: Vec<&str> = p.own_repos().filter_map(|r| r.language.as_deref()).collect();
    if langs.len() < 5 {
        return None;
    }
    let top = langs
        .iter()
        .max_by_key(|l| langs.iter().filter(|x| x == l).count())?;
    let count = langs.iter().filter(|x| x == &top).count();
    let pct = count * 100 / langs.len();
    if *top == "HTML" && pct >= 50 {
        return burn(format!("Top language: HTML ({pct}%). Bold to call that a language."), 1);
    }
    if pct < 80 {
        return None;
    }
    burn(format!("{pct}% {top}. Have you heard there are other languages?"), 1)
}

fn slow_starter(p: &Profile) -> Option<Burn> {
    let years = p.days_since(&p.user.created_at)? / 365;
    let own = p.own_repos().count();
    if years < 3 || own >= 5 || own == 0 {
        return None;
    }
    burn(format!("{years} years on GitHub, {own} repos. Pacing yourself, I see."), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{days_from_timestamp, User};

    const TODAY: &str = "2026-10-10T00:00:00Z";

    fn user() -> User {
        User {
            login: "someone".into(),
            name: None,
            bio: Some("Builds compilers for fun".into()),
            followers: 50,
            following: 10,
            public_repos: 0,
            created_at: "2025-01-01T00:00:00Z".into(),
        }
    }

    fn repo(name: &str) -> Repo {
        Repo {
            name: name.into(),
            description: Some("Does a thing".into()),
            language: Some("Rust".into()),
            fork: false,
            stargazers_count: 5,
            pushed_at: Some("2026-10-01T00:00:00Z".into()),
        }
    }

    fn profile(user: User, repos: Vec<Repo>) -> Profile {
        Profile { user, repos, today: days_from_timestamp(TODAY).unwrap() }
    }

    fn texts(p: &Profile) -> Vec<String> {
        judge(p).burns.into_iter().map(|b| b.text).collect()
    }

    fn mentions(p: &Profile, needle: &str) -> bool {
        texts(p).iter().any(|t| t.contains(needle))
    }

    #[test]
    fn a_clean_profile_gets_a_ten() {
        let p = profile(user(), vec![repo("parser"), repo("lexer"), repo("vm")]);
        let v = judge(&p);
        assert!(v.burns.is_empty(), "unexpected burns: {:?}", v.burns);
        assert_eq!(v.score, 10);
    }

    #[test]
    fn no_repos_is_a_ghost() {
        assert!(mentions(&profile(user(), vec![]), "ghost"));
    }

    #[test]
    fn only_forks_still_counts_as_a_ghost() {
        let mut fork = repo("linux");
        fork.fork = true;
        assert!(mentions(&profile(user(), vec![fork]), "ghost"));
    }

    #[test]
    fn spots_tutorial_projects() {
        let p = profile(user(), vec![repo("todo-app"), repo("Netflix-Clone"), repo("vm")]);
        assert!(mentions(&p, "2 tutorial projects (todo-app and Netflix-Clone)"));
    }

    #[test]
    fn spots_missing_descriptions() {
        let mut repos = vec![repo("a"), repo("b"), repo("c")];
        repos[0].description = None;
        repos[1].description = Some("   ".into());
        assert!(mentions(&profile(user(), repos), "2 of 3 repos have no description"));
    }

    #[test]
    fn one_missing_description_is_forgiven() {
        let mut repos = vec![repo("a"), repo("b"), repo("c")];
        repos[0].description = None;
        assert!(!mentions(&profile(user(), repos), "description"));
    }

    #[test]
    fn spots_the_graveyard() {
        let mut repos = vec![repo("a"), repo("b"), repo("c")];
        repos[0].pushed_at = Some("2020-01-01T00:00:00Z".into());
        repos[1].pushed_at = Some("2021-01-01T00:00:00Z".into());
        assert!(mentions(&profile(user(), repos), "2 repos untouched"));
    }

    #[test]
    fn spots_zero_stars() {
        let mut repos = vec![repo("a"), repo("b"), repo("c")];
        repos.iter_mut().for_each(|r| r.stargazers_count = 0);
        assert!(mentions(&profile(user(), repos), "0 stars"));
    }

    #[test]
    fn spots_unrequited_follows() {
        let mut u = user();
        u.following = 300;
        u.followers = 4;
        assert!(mentions(&profile(u, vec![repo("a")]), "Follows 300 people, followed by 4"));
    }

    #[test]
    fn spots_a_missing_bio_and_buzzwords() {
        let mut u = user();
        u.bio = None;
        assert!(mentions(&profile(u.clone(), vec![repo("a")]), "No bio"));
        u.bio = Some("Aspiring full-stack dev".into());
        assert!(mentions(&profile(u, vec![repo("a")]), "\"aspiring\""));
    }

    #[test]
    fn spots_one_language_and_html() {
        let rust: Vec<Repo> = ["a", "b", "c", "d", "e"].iter().map(|n| repo(n)).collect();
        assert!(mentions(&profile(user(), rust.clone()), "100% Rust"));

        let mut html = rust;
        html.iter_mut().take(3).for_each(|r| r.language = Some("HTML".into()));
        assert!(mentions(&profile(user(), html), "Top language: HTML (60%)"));
    }

    #[test]
    fn spots_lazy_names() {
        let p = profile(user(), vec![repo("test"), repo("Untitled"), repo("vm")]);
        assert!(mentions(&p, "Repos named test and Untitled"));
    }

    #[test]
    fn score_never_drops_below_one() {
        let mut u = user();
        u.bio = None;
        u.following = 500;
        u.followers = 0;
        u.created_at = "2015-01-01T00:00:00Z".into();
        let mut repos: Vec<Repo> = ["todo", "test", "calculator"].iter().map(|n| repo(n)).collect();
        for r in &mut repos {
            r.description = None;
            r.stargazers_count = 0;
            r.pushed_at = Some("2016-01-01T00:00:00Z".into());
        }
        let v = judge(&profile(u, repos));
        assert_eq!(v.score, 1);
    }
}
