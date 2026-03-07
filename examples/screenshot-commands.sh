#!/bin/bash
# Run these commands one by one in your terminal for the screenshot.
# cd to the pick repo first.

# JSON — live GitHub API
curl -s https://api.github.com/users/octocat | pick login

# .env file
cat examples/.env | pick DATABASE_URL

# YAML config
cat examples/config.yaml | pick server.port

# TOML
cat examples/config.toml | pick package.version

# HTTP headers — live
curl -sI https://example.com | pick content-type

# logfmt log line
echo 'level=error msg="connection refused" host=db.internal request_id=ghi-789' | pick request_id

# CSV
cat examples/users.csv | pick '[0].name'

# JSON — wildcard selector
cat examples/data.json | pick 'users[*].name' --lines
