# rcanary

[![Build Status](https://travis-ci.org/gyng/rcanary.svg?branch=master)](https://travis-ci.org/gyng/rcanary)

(In development) A minimal program to monitor statuses of hosts.

# Usage

   git clone https://github.com/gyng/rcanary.git
   cd rcanary
   cargo run my_config.toml

Configure the targets to probe in the configuration toml passed in to the program. An example is in `test/fixtures/config.toml`.

# License

  MIT. See `LICENSE` for details.
