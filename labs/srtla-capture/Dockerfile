# Linux lab running upstream SRTLA in network namespaces, to observe its wire behaviour.
# Reference only: nothing from this image is copied into freewheel.

# Two targets: `lab` (the default for run-all.sh) and `bench`, the lab plus
# irlserver/srtla_send (Rust, MIT), the other open source sender we benchmark against. BuildKit
# only builds irlserver for `bench`: it takes minutes.
FROM rust:1.94.0-slim-trixie AS irlserver
RUN apt-get update && apt-get install --yes --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*
ARG IRLSERVER_COMMIT=9e63d8fab313d876917102b0ae26823a45016d9e
RUN git clone https://github.com/irlserver/srtla_send.git /opt/irlserver \
	&& git -C /opt/irlserver checkout "${IRLSERVER_COMMIT}" \
	&& cargo build --release --locked --manifest-path /opt/irlserver/Cargo.toml

FROM debian:trixie-slim AS lab

RUN apt-get update \
	&& apt-get install --yes --no-install-recommends \
		build-essential ca-certificates git ffmpeg srt-tools tcpdump iproute2 iptables conntrack netcat-openbsd xxd procps pv \
	&& rm -rf /var/lib/apt/lists/*

ARG SRTLA_COMMIT=37862da3d0c13b46956efd3f88877053293d97d6
RUN git clone https://github.com/voila-voila-dev/srtla.git /opt/srtla \
	&& git -C /opt/srtla checkout "${SRTLA_COMMIT}" \
	&& make -C /opt/srtla

COPY lib /opt/lab/lib
COPY scenarios /opt/lab/scenarios
COPY run-scenario.sh /usr/local/bin/run-scenario
ENTRYPOINT ["run-scenario"]

FROM lab AS bench
COPY --from=irlserver /opt/irlserver/target/release/srtla_send /opt/irlserver/srtla_send
