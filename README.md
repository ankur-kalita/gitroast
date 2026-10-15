# gitroast

Judges a GitHub profile. Harshly.

```
$ gitroast torvalds

Roasting Linus Torvalds (@torvalds)

  🔥 No bio. Even your own profile has nothing to say about you.
  🔥 88% C. A one-trick pony, and the trick is mid.

  Score: ████████░░ 8/10
  Decent. That's the nicest insult I have.
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
| Empty repos | any repo with nothing ever pushed to it |
| Final-final-v2 | repo names with `final`, `v2`, `copy`, `backup`, `old`… |
| Dormant | no push to any repo for 6 months |
| Follower drought | 2+ years on GitHub, under 10 followers |

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

## Server mode

```
cargo run -- serve --port 8080
curl localhost:8080/roast/torvalds
```

| Route | Purpose |
|-------|---------|
| `GET /roast/{user}` | the roast as JSON (cached 10 minutes) |
| `GET /healthz` | liveness: the process is up |
| `GET /readyz` | readiness: 503 while shutting down |
| `GET /metrics` | Prometheus metrics |

Logs are one JSON line per request on stdout. On SIGTERM the server stops
being ready, waits 5 seconds, finishes in-flight requests and exits.

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
| `src/server.rs` | impure | `gitroast serve`: HTTP, health checks, metrics, logs |

The roast rules never go online, so `cargo test` checks them with fake profiles.

## Delivery and deployment

```
git push ──► CI: fmt + clippy ─┐
                 cargo test  ──┴─► docker build ─► smoke test ─► Trivy ─► ghcr.io/ankur-kalita/gitroast:<sha>
                                                                              │
              helm upgrade --set image.tag=<sha>  ◄───────────────────────────┘
                     │
                     ▼
              kind cluster: rolling update, 2 pods, Prometheus + Grafana
```

| Path | What |
|------|------|
| `Dockerfile` | multi-stage build, distroless runtime, non-root |
| `.github/workflows/ci.yml` | lint and test in parallel, then build, scan and publish the image |
| `charts/gitroast/` | Helm chart: Deployment, Service, optional monitoring |
| `deploy/monitoring-values.yaml` | settings for kube-prometheus-stack on kind |

### Prerequisites

Docker, [kind](https://kind.sigs.k8s.io/), kubectl and Helm 3.8+.
Rust is only needed to build or test outside Docker.

### 1. Create the cluster

```
kind create cluster --name gitroast
```

### 2. Install Prometheus and Grafana (optional, needed for monitoring)

```
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm upgrade --install monitoring prometheus-community/kube-prometheus-stack \
  -n monitoring --create-namespace -f deploy/monitoring-values.yaml --wait
```

### 3. Add a GitHub token (optional)

Without one, the app works at 60 GitHub requests/hour. A fine-grained token
with no permissions raises that to 5,000. It is never stored in git:

```
kubectl create namespace gitroast
kubectl -n gitroast create secret generic gitroast-github --from-literal=token=<token>
```

### 4. Deploy

Every push publishes an image tagged with its full commit SHA. Deploy one:

```
helm upgrade --install gitroast charts/gitroast -n gitroast --create-namespace \
  --set image.tag=<commit-sha> --set monitoring.enabled=true --wait
```

Leave out `monitoring.enabled=true` if step 2 was skipped. To run a local
build instead: `docker build -t gitroast:dev . && kind load docker-image
gitroast:dev --name gitroast`, then deploy with
`--set image.repository=gitroast --set image.tag=dev`.

### 5. Use it

```
kubectl -n gitroast port-forward svc/gitroast 8080:80
curl localhost:8080/roast/torvalds
```

Grafana (dashboard "gitroast"):

```
kubectl -n monitoring get secret monitoring-grafana -o jsonpath='{.data.admin-password}' | base64 -d; echo
kubectl -n monitoring port-forward svc/monitoring-grafana 3000:80   # http://localhost:3000, user admin
```

Prometheus (targets, alerts): `kubectl -n monitoring port-forward svc/monitoring-kube-prometheus-prometheus 9090:9090`

### Roll back a bad release

```
helm history gitroast -n gitroast
helm rollback gitroast <revision> -n gitroast --wait
```

Rolling updates use `maxUnavailable: 0`: a new version that never becomes
ready never replaces the running pods, so a bad image stalls instead of
causing an outage.

### What is running?

```
kubectl -n gitroast get deploy gitroast -o wide          # image tag = commit SHA
curl -s localhost:8080/metrics | grep gitroast_build_info
```

### Clean up

```
kind delete cluster --name gitroast
```

### Known limitations

- Deploys are a manual `helm upgrade`: GitHub's runners cannot reach a local
  kind cluster. The deployed tag lives in Helm's history, not in git, so
  manual changes to the cluster are not detected (GitOps would fix this).
- The cache is per pod and in memory: each replica fetches a user once.
- Alerts show in Prometheus only; there is no Alertmanager to notify anyone.
- No Ingress or TLS: access is through `kubectl port-forward`.
- Single-node cluster: no spreading across nodes or zones.
