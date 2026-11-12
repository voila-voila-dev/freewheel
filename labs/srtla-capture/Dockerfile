# Linux lab running upstream SRTLA in network namespaces, to observe its wire behaviour.
# Reference only: nothing from this image is copied into freewheel.
FROM debian:trixie-slim

RUN apt-get update \
	&& apt-get install --yes --no-install-recommends \
		build-essential ca-certificates git ffmpeg srt-tools tcpdump iproute2 iptables conntrack netcat-openbsd xxd procps \
	&& rm -rf /var/lib/apt/lists/*

ARG SRTLA_COMMIT=37862da3d0c13b46956efd3f88877053293d97d6
RUN git clone https://github.com/voila-voila-dev/srtla.git /opt/srtla \
	&& git -C /opt/srtla checkout "${SRTLA_COMMIT}" \
	&& make -C /opt/srtla

COPY lib /opt/lab/lib
COPY scenarios /opt/lab/scenarios
COPY run-scenario.sh /usr/local/bin/run-scenario
ENTRYPOINT ["run-scenario"]
