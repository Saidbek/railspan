# dummy_rails

Minimal Rails API app for dogfooding Railspan.

## Setup

```bash
bundle install
bin/rails db:prepare db:seed
```

## Run with stdout exporter

```bash
RAILSPAN_EXPORTER=stdout bin/rails server -p 3000
curl localhost:3000/users
curl localhost:3000/users/with_posts
```

## Run with agent

```bash
# terminal 1
cargo run -p railspan-cli -- serve

# terminal 2
RAILSPAN_EXPORTER=http RAILSPAN_ENDPOINT=http://127.0.0.1:7421 bin/rails server -p 3000
```
