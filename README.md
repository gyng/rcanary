# rcanary

[![Build Status](https://travis-ci.org/gyng/rcanary.svg?branch=master)](https://travis-ci.org/gyng/rcanary)

A minimal program to monitor statuses of webpages, with super-basic logging and email alerts via SMTP. Supports basic auth for HTTP targets. rcanary exposes a websocket server for dashboards to connect to.

# Usage

    git clone https://github.com/gyng/rcanary.git
    cd rcanary
    cargo run --release my_config.toml

Configure settings and the targets to probe in the configuration toml passed in to the program. An example is in [`tests/fixtures/config.toml`](tests/fixtures/config.toml).

## Gmail
SMTP configuration for Gmail can be found [here](https://support.google.com/a/answer/176600). Additional details on using Gmail SMTP can be found [here](https://www.digitalocean.com/community/tutorials/how-to-use-google-s-smtp-server). You might also need to [enable less secure apps](https://support.google.com/accounts/answer/6010255?hl=en). The example [`config.toml`](tests/fixtures/config.toml) has some defaults set for Gmail.

## Docker

By default, the image will mount a volume at `/app/config` and use `/app/config/config.toml`. Note that the configuration file is assumed to be at `config/config.toml` on the host.

Then, you can run it as such using:

    docker build -t rcanary .
    docker run -v /path/to/config:/app/config rcanary

    # Or use docker-compose
    docker-compose up

You will need at least Docker engine version 17.05 (API version 1.29) to build the image.

## Logging

All log output is sent to `stdout`. The Docker image also `tee`s the log output into files in the `logs` volume. To do it without Docker, pipe the output into a file with `tee`:

    cargo run --release -- /app/config/config.toml | tee "/app/logs/`date +%s`.log"

Note: the logger overrides `RUST_LOG` to be `info`.

## Dashboard

An example dashboard is at [`src/dashboard/index.html`](src/dashboard/index.html). By default it connects to port `8099` on the current hostname.

    http://localhost
    connects to => ws://localhost:8099

    https://my.dashboard.example.com
    connects to => wss://my.dashboard.example.com:8099

To specify a rcanary instance to connect to, add a hash to the URL as such:

    http://my.dashboard.example.com#ws://my.rcanary.example.com:8888
    connects to => ws://my.rcanary.example.com:8888

# License

MIT. See `LICENSE` for details.
