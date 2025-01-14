#!/usr/bin/env bash

# Download the bootstrap script
curl -sf https://gitlab.redox-os.org/redox-os/redox/raw/master/bootstrap.sh -o bootstrap.sh
# Update Ubuntu/Debian-based systems
bash -e bootstrap.sh -d
# Update the Rust toolchain
rustup update || true
# Update the build system source and submodules
make pull
# Update the relibc folder timestamp
touch relibc
# Update relibc
make prefix
# Update recipes
make rebuild