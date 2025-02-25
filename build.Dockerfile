# Copyright 2025 Sean Kelleher. All rights reserved.
# Use of this source code is governed by an MIT
# licence that can be found in the LICENCE file.

FROM rust:1.83.0-bullseye

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN \
    rustup component add \
        clippy

RUN \
    apt-get update \
    && apt-get install \
        --assume-yes \
        git \
        make \
        python3 \
        python3-pip \
        wget

RUN \
    wget \
        https://github.com/eZanmoto/dpnd/releases/download/v0.1.14/dpnd-v0.1.14-x86_64-unknown-linux-gnu.tar.gz \
        --output-document=/tmp/dpnd.tar.gz \
    && tar \
        --extract \
        --directory=/tmp \
        --file=/tmp/dpnd.tar.gz \
    && mv \
        /tmp/dpnd-v0.1.14-x86_64-unknown-linux-gnu \
        /usr/local/bin/dpnd \
    && pip3 install \
        comment-style===0.1.0

RUN \
    curl \
        --fail \
        --silent \
        --show-error \
        --location \
        'https://just.systems/install.sh' \
    | bash \
        -s \
        -- \
        --tag 1.38.0 \
        --to /usr/local/bin
