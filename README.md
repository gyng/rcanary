# rcanary

[![Build Status](https://travis-ci.org/gyng/rcanary.svg?branch=master)](https://travis-ci.org/gyng/rcanary)

A minimal program to monitor statuses of webpages, with super-basic logging and email alerts via SMTP. rcanary exposes a websocket server for dashboards to connect to.

# Usage

    git clone https://github.com/gyng/rcanary.git
    cd rcanary
    cargo run --release my_config.toml

Configure settings and the targets to probe in the configuration toml passed in to the program. An example is in `test/fixtures/config.toml`.

## Gmail
SMTP configuration for Gmail can be found [here](https://support.google.com/a/answer/176600). Additional details on using Gmail SMTP can be found [here](https://www.digitalocean.com/community/tutorials/how-to-use-google-s-smtp-server). You might also need to [enable less secure apps](https://support.google.com/accounts/answer/6010255?hl=en).

## Docker

By default, the image will mount a volume at `/app/config` and use `/app/config/config.toml`. Note that the configuration file is assumed to be at `config/config.toml` on the host.

Then, you can run it as such using:

    docker build -t rcanary .
    docker run -v /path/to/config:/app/config rcanary
    
    # Or use docker-compose
    docker-compose up

## Logging

All log output is sent to `stdout`. The Docker image also `tee`s the log output into files in the `logs` volume. To do it without Docker, pipe the output into a file with `tee`:

    cargo run --release -- /app/config/config.toml | tee "/app/logs/`date +%s`.log"

Note: the logger overrides `RUST_LOG` to be `info`.

## Dashboard

An example dashboard is at `src/dashboard/index.html`. Point `serverAddress` in `rcanary.js` to your rcanary server.

# License

MIT. See `LICENSE` for details.
