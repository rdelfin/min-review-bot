# min-review-bot

[![Rust](https://github.com/rdelfin/min-review-bot/actions/workflows/rust.yaml/badge.svg?branch=main)](https://github.com/rdelfin/min-review-bot/actions/workflows/rust.yaml)

Do you have overly complex owner rules in your Github repo? Is it hard to figure
out who the minimum set of owners needed for a review are? This tool helps you
solve this issue. It will:

- Scan through all the PRs on a given repo for the set of owners required
- Compute the real minumum set of reviewers required to merge
- Leave a comment on the PR showing the corresponding boolean expression

It can be configured to run on any machine, and only requires a GitHub app as
well as a SQLite DB which can be re-generated on restart.

## Configuration


We recommend setting this up with docker compose. We provided a sample `docker-compose.yaml` file in the root directory, but you basically want to put:

```yaml
version: '3'
services:
  daemon:
    image: rdelfin/min_review_bot:latest
    volumes:
      - ./config.toml:/etc/reviewbot/config.toml:ro
      - ./private_key.pem:/etc/reviewbot/private_key.pem:ro
      - ./data:/var/cache/reviewbot:rw
    restart: always
    network_mode: "host"
    command: ["min_review_daemon", "--config", "/etc/reviewbot/config.toml"]
```

And this should be accompanied by a matching `config.toml` file in that directory:

```toml
repo = "YOUR_REPO"
bot_username = "YOUR BOT'S USERNAME"
sleep_period = { secs = 60, nanos = 0 }
# This should be a list of users that have this bot enabled. We will change this
# to a blocklist in a future release
users = [
    "user1",
    "user2",
]
db_path = "/var/cache/reviewbot/data.db"

[github]
private_key_path = "PATH_TO_GITHUB_APP_PEM_FILE"
app_id = YOUR_GITHUB_APP_ID
```

You can then run:

```bash
$ docker compose -d
```

And you should have a running application.

### Creating a Github application

As you can see, this requires creating a github app to put down the comments in
your place. We recommend following
[this guide](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/about-creating-github-apps)
to create the app. You will need a github APP ID as well as a `.pem` file to
authenticate. Place these in the appropriate fields in the config.

### Storing private key

The private key can be stored in a file or as an environment variable. If the
`private_key_path` field on the `config.toml` isn't set, we'll read the private
key contents from the `GITHUB_PRIVATE_KEY` environment variable directly.
