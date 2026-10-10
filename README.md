# gitroast

Judges a GitHub profile. Harshly.

```
$ gitroast torvalds

Roasting Linus Torvalds (@torvalds)

  🔥 No bio. Letting the code speak. The code is quiet.
  🔥 88% C. Have you heard there are other languages?

  Score: ████████░░ 8/10
  Decent. A few skeletons in the repo closet.
```

## What it judges

| Rule | Triggers when |
|------|---------------|
| Ghost | no original repos at all |
| Missing descriptions | half or more of the repos have none |
| Tutorial projects | 2+ repos named like `todo`, `calculator`, `netflix-clone`… |
| Lazy names | repos called `test`, `untitled`, `asdf`… |
| Graveyard | half or more of the repos untouched for a year |
| Zero stars | 3+ repos and not a single star |
| Fork collector | more forks than originals |
| Unrequited | following 3× more people than follow back |
| Bio | no bio, or buzzwords like "passionate" or "aspiring" |
| One language | 80%+ of repos in one language (50%+ if it's HTML) |
| Slow starter | 3+ years on GitHub, under 5 repos |

Every profile starts at 10. Each roast costs 1–5 points; the score never goes below 1.

## Usage

```
cargo run -- <username>
```

Uses the public GitHub API: no login needed, 60 requests/hour. For more, set a
token:

```
export GITHUB_TOKEN=ghp_...
```

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Roast delivered |
| `1` | User not found, rate limited, or network problem |
| `2` | Not a valid username |

## How it's put together

| File | Purity | Job |
|------|--------|-----|
| `src/profile.rs` | pure | `User`, `Repo` and `Profile` types, date parsing |
| `src/roast.rs` | pure | the rules: profile in, roasts and score out |
| `src/github.rs` | impure | the only file that talks to the internet |
| `src/main.rs` | impure | command line, coloured output, exit codes |

The roast rules never go online, so `cargo test` checks them with fake profiles.
