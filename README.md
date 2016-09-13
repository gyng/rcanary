# rcanary

[![Build Status](https://travis-ci.org/gyng/rcanary.svg?branch=master)](https://travis-ci.org/gyng/rcanary)

A minimal program to monitor statuses of webpages, with super-basic logging and email alerts via SMTP. rcanary exposes a websocket server for dashboards to connect to.

# Usage

    git clone https://github.com/gyng/rcanary.git
    cd rcanary
    cargo run --release my_config.toml

Configure settings and the targets to probe in the configuration toml passed in to the program. An example is in `test/fixtures/config.toml`. [SMTP configuration for Gmail can be found here](https://support.google.com/a/answer/176600
).

## Docker

By default, the image will mount a volume at `/app/src/config` and use `/app/src/config/config.toml`.

Then, you can run it as such:

    docker build -t rcanary .
    docker run -v /path/to/config:/app/src/config rcanary
    
    # Or use docker-compose
    docker-compose up

## Dashboard

An example dashboard is at `src/dashboard/index.html`. Point `serverAddress` in `rcanary.js` to your rcanary server.

# License

MIT. See `LICENSE` for details.
