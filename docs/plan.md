# gitroast — plan

`gitroast <username>` looks at a public GitHub profile and judges it.

## What it looks at
From the public GitHub API (no login needed, 60 requests/hour; set
`GITHUB_TOKEN` for more):
- the profile: bio, followers, following, account age
- the public repos: names, descriptions, languages, stars, forks, last push

## What it prints
- a list of roast lines, each triggered by a rule ("12 repos, no descriptions")
- a final score out of 10 and a one-line verdict

## How the code is split
| File | Purity | Job |
|------|--------|-----|
| `src/profile.rs` | pure | `Profile` and `Repo` types |
| `src/roast.rs`   | pure | rules: profile in, roast lines + score out |
| `src/github.rs`  | impure | the only file that talks to the internet |
| `src/main.rs`    | impure | command line, printing, exit codes |

The roast rules never go online, so they are tested with fake profiles.

## Steps (one commit each)
1. Project skeleton and this plan
2. Profile types
3. Roast rules + tests
4. GitHub fetching
5. Command line + nice output
6. README

Out of scope for now: CI/CD, Docker, AI-written roasts.
