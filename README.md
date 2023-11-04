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
