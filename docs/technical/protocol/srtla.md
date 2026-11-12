# SRTLA v2 protocol

What freewheel must do to interoperate with BELABOX senders and receivers. Written in our own
words from two sources:

- **Observed**: packet captures of the upstream binaries in the lab (`labs/srtla-capture/`),
  analysed by `analyze-srtla-capture` (`crates/packet-capture`). Facts marked *(observed)* come
  from those captures; those marked with a test name are checked on a real capture in CI.
- **Read**: upstream `srtla` at commit `37862da3d0c13b46956efd3f88877053293d97d6`, read for
  intent only. Upstream is AGPL-3.0: nothing here is code, and nothing may be copied from it.

## Roles

```
SRT caller ─udp─▶ sender (box) ══ N links, one UDP socket per source IP ══▶ receiver (server) ─udp─▶ SRT listener
```

- The **sender** listens on a local UDP port for one SRT caller and forwards every datagram,
  unchanged, on one of its links. It binds one UDP socket per source address (one per modem)
  and relies on source routing so each socket leaves through its modem.
- The **receiver** listens on one UDP port, gathers the links of one sender into a
  **connection group**, and forwards everything to the SRT listener through one UDP socket per
  group. Replies from SRT come back on that socket.
- SRT packets are never modified. SRTLA adds its own control packets, which never reach SRT.
- IPv4 only, one SRT stream per sender.

## Telling packets apart

Every datagram starts with 32 bits, big-endian.

- **Top bit 0**: SRT data packet; the 31 low bits are its sequence number.
- **Top bit 1**: control packet; the first 16 bits are its type.

| Type | Name | Length (bytes) | Direction | After the type |
|---|---|---|---|---|
| `0x8000` | SRT handshake | 64 or 80 *(observed)* | both | SRT |
| `0x8001` | SRT keepalive | SRT | both | SRT, forwarded as is |
| `0x8002` | SRT ACK | SRT | server → box | SRT; bytes 16–19: last acknowledged sequence number |
| `0x8003` | SRT NAK | SRT | server → box | SRT; loss list (below) |
| `0x8005` | SRT shutdown | SRT | both | SRT |
| `0x8006` | SRT ACKACK | SRT | box → server | SRT, forwarded as is |
| `0x9000` | SRTLA keepalive | 2 | both | nothing |
| `0x9100` | SRTLA ACK | 44 *(observed)* | server → box | 2 zero bytes, then 10 × 32-bit sequence numbers |
| `0x9200` | REG1 | exactly 258 | box → server | 256-byte group id proposal |
| `0x9201` | REG2 | exactly 258 | both | 256-byte group id |
| `0x9202` | REG3 | exactly 2 | server → box | nothing |
| `0x9210` | REG_ERR | 2 | server → box | nothing |
| `0x9211` | REG_NGP | 2 | server → box | nothing: "no such group" |
| `0x9212` | REG_NAK | 2 | — | defined, never sent |

Registration packets of any other length are not registration packets. The receiver drops
anything shorter than 16 bytes that is not an SRTLA packet.

### How `bonding-protocol` decodes

`Packet::decode` (`crates/bonding-protocol/src/srtla/packet.rs`) applies the rules above to
every datagram, the same way in both directions, and `Packet::encode` writes it back byte for
byte. Where upstream is vaguer, it is stricter, never looser:

- Over 1 500 bytes: refused (upstream truncates to its buffer).
- Keepalive: any length; the bytes after the type are kept, since upstream echoes them.
- SRTLA ACK: must be zero-padded after the type and made of whole 31-bit numbers, else
  refused. Upstream reads whatever words are there.
- REG_ERR, REG_NGP, REG_NAK: exactly 2 bytes. Upstream's sender accepts any length.
- An SRTLA type of the wrong length, 16 bytes or more: an SRT control packet, forwarded, as
  upstream's receiver does.
- SRT ACK under 20 bytes, or NAK with a ragged or unclosed loss list: still forwarded as SRT
  control, just not read. Upstream reads past the end of such packets.

Checked by property tests (`decode(encode(x)) == x`), a seeded fuzz test (every input up to
2 bytes, every type at every boundary length, 2 million mutations in release) and the lab:
all 1 965 784 datagrams of the 35 scenarios decode and encode back unchanged; the only one
refused is a 1-byte hostile packet (`crates/packet-capture/tests/lab/wire_format.rs`).

SRT NAK loss list (from byte 16, 32-bit words): a word with the top bit clear is one lost
sequence number; a word with the top bit set starts a range: its low 31 bits are the first
lost number, the next word the last one (inclusive).

## Registration

The group id is 256 bytes: the first 128 are chosen by the sender, the last 128 by the
receiver. Together they are the only secret proving that a link belongs to a group.

```
sender                                         receiver
  │ REG2(random id), every link        ──────▶  unknown group
  │                                    ◀──────  REG_NGP
  │ REG1(sender half…), on that link   ──────▶  creates the group, fills its half
  │                                    ◀──────  REG2(full id)
  │ REG2(full id), every link          ──────▶  adds each link to the group
  │                                    ◀──────  REG3, per link
```

*(test `registration_follows_reg2_ngp_reg1_reg2_reg3`; same order in every scenario)*

Sender:

1. At start every link counts as failed, so the 1-second housekeeping sends REG2 with the
   sender's random 256-byte id on every link. *(observed: REG2 every ~1 s on both links while
   the receiver is down)*
2. On REG_NGP, and only if no link is established and no REG1 is pending, it sends REG1 on
   that link. REG1 stays pending for 4 s.
3. On REG2 for the pending link, it checks that the first 128 bytes are its own, adopts the
   full id and sends REG2 on every link. *(test
   `receiver_keeps_the_sender_half_of_the_group_id`)*
4. Each REG3 marks its link established. *(test `every_link_gets_its_own_reg3`)*
5. A link that fails later is re-registered with REG2 alone, using the full id.

REG_NGP and REG2 do not count as activity on a link: they must not keep a dead link alive.

Receiver:

- **REG1**: refused (REG_ERR) when 200 groups exist or when the source address already belongs
  to a group. Otherwise it creates the group from the sender's 128 bytes plus 128 random bytes
  and answers REG2.
- **REG2**: unknown id → REG_NGP. Address already in another group → REG_ERR. Group full
  (8 links) → REG_ERR. Otherwise it adds the link (or keeps it, if already there) and answers
  REG3. Re-registering a known link is allowed and harmless.
- A link is identified by source address **and port**: a NAT rebinding is a new link.
- Any other packet from an address that is not a registered link is dropped **silently**.

## Keepalives

- The sender sends a keepalive (`0x9000`) on a link when it has **sent** nothing on it for more
  than 1 s; links carrying data never send keepalives. *(observed: keepalives only after the
  stream ends, or on a link the scheduler stopped using; gaps of ~1 s)*
- The receiver echoes the same 2 bytes on that link and refreshes it. *(observed)*
- The echo is the sender's only proof that an idle link works. Upstream measures no RTT with it.

## Acknowledgements

Two kinds travel back to the sender.

**SRTLA ACK, per link.** For each link, the receiver remembers the sequence numbers of the data
packets that arrived on it. At every 10th, it sends an SRTLA ACK listing those 10 numbers on
that same link. *(test `srtla_acks_carry_ten_sequence_numbers_from_the_same_link`)* A partial
batch is never flushed: the last 0–9 packets of a link never get an SRTLA ACK.

**SRT traffic from the listener.**

- SRT ACKs go on **every** link of the group. *(test `srt_acks_are_sent_on_every_link`)*
- Everything else (NAKs, handshakes, keepalives, shutdown) goes on the link that most recently
  carried a packet to the receiver.

The sender forwards every SRT packet to the SRT caller, and reads ACKs and NAKs on the way to
steer its links.

## Link selection (sender)

Each link has:

- `window`, in thousandths of a packet: starts at 20 000, never below 1 000 or above 60 000;
- `in_flight`: packets sent and not yet acknowledged;
- a log of the last 256 sequence numbers sent on it.

For each datagram from the SRT caller, the sender picks the usable link with the highest
`window / (in_flight + 1)` (integer division; on a tie, the first link in its list keeps it).
*Usable* means it heard anything from the receiver in the last 4 s except `REG_NGP` and REG2,
so a REG_ERR makes a link usable and a REG3 is not required. With no usable link, the
datagram is **dropped**. Data packets are logged on the chosen link;
control packets are sent without being logged.

| Event | Effect on the window |
|---|---|
| SRTLA ACK of a number found in a link's log | that link: `in_flight − 1`; if `in_flight × 1000 > window`, `window + 29` |
| Each number in an SRTLA ACK, found or not | **every** link that has ever received something: `window + 1`, capped |
| SRT NAK of a number found in a link's log | that link: `window − 100`, floored |
| SRT ACK of number *n* | every link: `in_flight` = logged numbers ≥ *n* |
| Link declared failed | `window` = 1 000, `in_flight` = 0, log cleared |

*(observed: two equal links split 47 % / 53 %)*

## Timeouts and limits

| What | Value | Side |
|---|---|---|
| Housekeeping | every 1 s | sender |
| Link failed | nothing received for more than 4 s (seconds resolution, so 4–5 s) | sender |
| Link failed at once | a send error on that link | sender |
| REG1 pending | 4 s | sender |
| All links failed | 10 s → exit (before the first connection: next resolved address) | sender |
| Link forgotten | nothing received for 10 s, checked every 3 s (so 10–13 s) | receiver |
| Group without links removed | 10 s after creation | receiver |
| Links per group | 8 | receiver |
| Groups | 200 | receiver |
| Largest datagram | 1 500 bytes | both |

## Behaviour by scenario

Lab (`labs/srtla-capture/`): box, network and server in separate namespaces, `tc netem` per
link and direction, 3 Mbit/s H.264 test pattern, SRT latency 2 000 ms, 20 ms one-way delay by
default. Each line is checked by a test in `crates/packet-capture/tests/lab/<scenario>.rs`.
Shares are of data packets sent by the box. Every line held over at least two runs; thresholds
in the tests leave room for run-to-run spread. The lab is CPU-sensitive: an overloaded host
(e.g. a build running beside it) paces the encoder unevenly and invalidates a run.

| Scenario | What upstream does |
|---|---|
| `steady` | Two equal links split ~45 / 55 %. No video lost. |
| `four-links` | Four equal links take 23–27 % each. |
| `asymmetric-links` | 8 / 2 / 0.5 Mbit/s links: the fast one takes **~96 %**; the 2 Mbit/s link, able to carry ~40 %, gets **~1 %**. |
| `low-bitrate` | 300 kbit/s: both links kept, ACKs still in batches of 10. |
| `congested` | 2 × 1.5 Mbit/s for 4 Mbit/s: **~38 % of packets dropped** by SRT. Nothing adapts the bitrate: that is the encoder's job. |
| `random-loss` | 5 % loss on link 2: its share falls to ~30 %; SRT repairs every loss. |
| `burst-loss` | Bursty loss on link 2: its share keeps falling (45 → ~20 %). |
| `jitter-reordering` | 60 ± 40 ms jitter, nothing lost: link 2 falls to **~2 %**. Reordering reads as loss (NAKs). |
| `bufferbloat` | 2 Mbit/s with a deep buffer: link 2 gets ~18 %. |
| `latency-spike` | 1.5 s one-way for 5 s, then normal again: link 2 stays at **~10 %** for good. |
| `link-drop` | 10 s dead zone: **~4 s of packets sent into the dead link**, then REG2 every second; after it returns, 1 % → ~15 % over 25 s (47 % before). |
| `link-drop-long` | 20 s: the receiver forgets the link, which must register again (second REG3); after that, ~9 % 14 s later. |
| `link-flapping` | 1 s down every 3 s: link 2 starved at **~4 %**. |
| `modem-unplugged` | Sends fail: the link is dropped at once, no loss; same slow ramp when it returns. |
| `uplink-only-drop` | Uplink dead, downlink fine: **never detected**. SRT ACKs, sent on every link, keep it alive; the box sends into it for the whole 10 s. |
| `downlink-only-drop` | Downlink dead, uplink fine: the link is dropped after 4 s although its data was arriving. |
| `all-links-drop` | 6 s tunnel: everything comes back by itself. |
| `all-links-lost` | 25 s tunnel: **`srtla_send` exits** ~16 s after the cut and nothing comes back without a supervisor. |
| `nat-rebinding` | Carrier NAT changes link 2's port: the server sees a new, unknown link and drops it silently; the box re-registers after its 4 s timeout, then the slow ramp. |
| `address-change` | Modem gets a new IP, list reloaded (SIGHUP): the new address starts at the **default window** and takes ~40 % at once, far faster than a recovering link. |
| `link-added` / `link-removed` | SIGHUP: a new modem takes ~30 % within seconds; a removed one stops at once. |
| `cold-start` | Receiver 6 s late: REG2 every second; the box **drops** all video until registration. |
| `receiver-restart` | New `srtla_rec`: the box keeps sending into the void for ~5 s, then REG_NGP → new group. |
| `sender-restart` | New id and ports: a second group; the old one lingers on the server until it times out. |
| `srt-server-restart` | The SRT listener restarts: `srtla_rec` **destroys the group**; the stream stops until the SRT caller reconnects (~5 s). |
| `too-many-links` | 9 links, 8 allowed: the 9th gets REG_ERR, which the box counts as "heard from the server", so it **sends data into it** for a few seconds, which the receiver throws away. |
| `two-senders` | Two boxes, two groups, kept apart. |
| `group-limit` | 200 fake REG1 from one address fill the receiver: the real box is refused (REG_ERR) until they expire 10–13 s later. Repeating the flood would lock it out for good. |
| `varying-bandwidth` | Link 2's capacity moves 8 → 1 → 4 → 0.5 → 8 Mbit/s: when it slows down it **keeps its traffic** (0.8–1.8× its capacity at 1 Mbit/s) until losses pile up, paid in retransmissions; a link that only slowed down, without dying, is back near a fair share **12–20 s** after its capacity returns (8 runs). |
| `real-ride` | Two minutes, 6 / 3 / 2 Mbit/s lossy, jittery links with a dead zone, a handover, a weak cell: the best link carries **~77 %** (fair: ~55 %); when links 1 and 2 die for 4 s, the starved link 3 is flooded at ~2.5× its capacity and **video is dropped** (~7 ‰). |
| `long-run` | Ten minutes, two links: memory of both processes flat, no stall, no drop. |
| `same-carrier` | Two SIMs behind one CGNAT address: told apart by port, ~50 / 50 %. |
| `group-hijack` | Anyone who sniffs the group id (sent in clear in REG2) joins the group from any address: REG3, then the stream's **SRT ACKs leak** to them and their **injected packets are forwarded** to the SRT server. |
| `hostile-packets` | Strangers get silence (short packets, wrong lengths, keepalives, data, REG3) except REG_NGP for an unknown REG2 and a group for a REG1; a second REG1 from the same address gets REG_ERR. The real stream is unaffected. |

## Where freewheel can do better

Design inputs for M1b (`freewheel.md`). Compatibility mode reproduces upstream as is.

- **Capacity-blind scheduling**: slow, jittery or bloated links are starved far below what they
  can carry (`asymmetric-links`, `jitter-reordering`, `bufferbloat`). Estimate each link's
  capacity and delay; treat reordering as reordering, not loss.
- **No recovery**: a link that had trouble once (`latency-spike`, `link-drop`, `link-flapping`)
  stays near zero for tens of seconds, while a brand-new address gets a full share at once.
  Probe returning links (shadow warm-up), then ramp on measured capacity.
- **Slow dead-link detection**: ~4 s of packets into a dead link. Per-packet ACK timing can flag
  it in well under a second.
- **One-way failures**: an uplink-only failure is never detected; a downlink-only failure kills
  a working link. Judge each direction by its own evidence (per-link data ACKs, not broadcast
  SRT ACKs).
- **Unknown links**: the receiver drops their packets silently (NAT rebinding, receiver
  restart). Answering "unknown link" lets the box re-register at once.
- **Giving up**: the sender exits after 10 s without links, and the receiver destroys the group
  when SRT hiccups. Never give up: keep the session, buffer, retry.
- **No buffering before registration**: the first seconds of video are lost on a cold start.
- **REG_ERR counted as activity**: a refused link is used as a black hole.
- **Partial ACK batches**: never flushed, so at low bitrate `in_flight` stays inflated.
- **RTT**: keepalives and ACKs carry no timestamps; nothing measures per-link latency.
- **Overflow on slowdown**: a link whose capacity drops keeps its window until losses pile up
  (`varying-bandwidth`), and the fallback after an outage floods whatever is left (`real-ride`).
  Pace each link at its measured rate.
- **Security**: the 256-byte id is the only credential and travels in clear, so whoever sniffs
  it joins the group, reads its ACKs and injects packets (`group-hijack`); no encryption; 200
  REG1 from a single address lock every box out (`group-limit`). Authenticate every link with
  a key the network never sees.
- **IPv4 only**, and a link changes identity when NAT changes its port.

## How `bonding-receiver` behaves

`crates/bonding-receiver` is the compatibility receiver: `bonding-receiver LISTEN_PORT SRT_HOST
SRT_PORT`, a drop-in for `srtla_rec`. Its state machine (`src/srtla/receiver.rs`) has no
sockets and no clock; the server (`src/server/`) runs it on one thread with `poll(2)`.
It reproduces upstream as is, including these details, read from upstream and not visible in
the captures:

- **Who owns an address**: groups are searched newest first; in each, its links first, then
  the group's *last address*. An address that is only a group's last address (e.g. the one
  that sent REG1) is "in use" for REG1 and REG2 of another group, but its other packets are
  dropped: it is not a link.
- **Last address**: set by REG1, by every REG3 sent, and by every packet of 16 bytes or more
  from a link. Keepalives and shorter packets do not move it.
- **Activity**: every packet from a link refreshes it, except REG1 and REG2. Re-registering a
  link does not keep it alive.
- **Keepalive**: type `0x9000` at any length, echoed whole on its link, never forwarded.
- **Forwarded to SRT**: anything else of 16 bytes or more from a link, byte for byte, SRTLA
  types included (a 44-byte SRTLA ACK from a sender would reach SRT). Shorter: dropped.
- **SRT socket**: one connected UDP socket per group, opened on the first forwarded packet, so
  SRT cannot answer before. Failing to open it, a failed send, a read error or a read under
  16 bytes destroys the group.
- **SRT ACK** (sent on every link): raw type `0x8002` at any length.
- **Cleanup**: at start, then at most every 3 s. A link goes when its last activity is more
  than 10 s old; a group without links when it is more than 10 s old.
- **Group id**: the receiver half comes from `/dev/urandom`, drawn again if the full id
  already exists; ids are compared in constant time.
- **I/O**: datagrams over 1 500 bytes are truncated to 1 500. Each wake reads one datagram
  from each ready socket, then runs the cleanup; the wait is at most 1 s.

Differences, none visible to a sender: no start-up SRT handshake probe (upstream sends one
and only warns if it fails), our own log lines, seconds counted from process start.

Checked by `crates/bonding-receiver/tests` (one file per topic, 100 000 random operations
for the limits, zero allocation per packet, a real-socket test) and by the lab: with
`RECEIVER=freewheel`, the 35 scenarios run with our receiver and the characterisation tests
of `crates/packet-capture/tests/lab/` are run on those captures.

## How `bonding-sender` behaves

`crates/bonding-sender` is the compatibility sender: `bonding-sender SRT_LISTEN_PORT SRTLA_HOST
SRTLA_PORT BIND_IPS_FILE`, a drop-in for `srtla_send`, `SIGHUP` included. Its state machine
(`src/srtla/sender.rs`) has no sockets and no clock; the server (`src/server/`) runs it on one
thread with `poll(2)`. It reproduces upstream as is, including these details, read from
upstream and not visible in the captures:

- **Link list**: one address per line, a dotted quad at the start of the line (whatever
  follows a blank is ignored; `255.255.255.255` is refused, as `inet_addr` does). Each new
  address goes to the **front** of the list, so the last line is first: that order breaks
  ties when choosing a link and is the order links are read. On `SIGHUP` the file is reread:
  known addresses keep their link and state, new ones start at the default window (their
  socket is bound at the next housekeeping), missing ones are closed at once. An unreadable
  file at that moment stops the sender.
- **Sockets**: one per address, bound to it on an ephemeral port, non-blocking, 8 MB send
  buffer asked. A full buffer is a failed send. A socket that cannot be bound is retried every
  housekeeping, quietly.
- **Activity**: every packet from the receiver refreshes its link, except `REG_NGP` and REG2
  (at any length). A failed data send makes the link unusable at once; the next housekeeping
  declares it failed.
- **Link failed** (housekeeping): window 1 000, nothing in flight, log emptied; then every
  second REG2 with the current id, or REG1 again if it is the link whose REG1 is pending.
- **Registration**: `REG_NGP` (any length) sends REG1 only when the last housekeeping counted
  no usable link and no REG3 arrived since, no REG1 is pending, and the last registration step
  is more than 4 s old. A REG2 counts only on the link that sent REG1. REG3 at any length
  establishes.
- **Keepalive**: at housekeeping, on a usable link not chosen for data in the current or the
  previous whole second. Sending a keepalive does not count as sending, so an idle link sends one every
  second.
- **Packet log**: 256 slots per link, all holding sequence number 0 when the link is created
  (emptied when it fails); searches go newest first and skip the oldest slot.
- **SRTLA ACK**: each number frees the newest logged copy on the first link holding it
  (listed order, not the link it came on); every link that ever heard from the receiver then
  grows by 1. The +29 for a link over its window is not capped.
- **SRT ACK** of *n*: on every link, slots below *n* are emptied and in flight becomes the
  count of the others (plain integer comparison: sequence wrap-around is not handled).
- **SRT NAK**: each lost number hits its first logged copy, links in list order, newest
  first. We walk the logs instead of the numbers, which gives the same result in time bounded
  by the logs, whatever the range.
- **Back to the SRT caller**: everything from the receiver except keepalives, SRTLA ACKs,
  REG3, `REG_NGP` and REG2. REG_ERR, REG_NAK and SRTLA types of odd lengths reach the caller,
  as upstream sends them. The caller's address is the source of its last datagram; nothing goes
  back before its first.
- **Giving up**: after more than 10 s with no usable link, the sender exits with status 1
  once it has been connected; before that, it moves to the receiver's next resolved address,
  and exits when none is left.
- **I/O**: datagrams over 1 500 bytes are truncated to 1 500. Each turn: reload if asked,
  housekeeping (at most once a second), wait at most 200 ms, then one datagram from the SRT
  caller and one from each ready link, in list order.

Differences, none visible to the receiver: malformed ACKs and NAKs (top bit set where a
sequence number belongs, ragged loss lists) are not read, where upstream reads past them or
past the packet; a REG2 of the wrong length on the pending link is ignored, where upstream
copies whatever its buffer holds; short forms of addresses (`10.1`, hex, octal) are not
accepted in the link list; our own log lines; seconds counted from process start.

Checked by `crates/bonding-sender/tests` (one file per topic, 100 000 random operations, zero
allocation per packet, a real-socket test through `bonding-receiver`) and by the lab: with
`SENDER=freewheel`, the 35 scenarios run with our sender against upstream's receiver and the
characterisation tests of `crates/packet-capture/tests/lab/` are run on those captures.

## Reproducing

```sh
labs/srtla-capture/run-all.sh                                                         # ~13 min
cargo run --release -p packet-capture --bin analyze-srtla-capture -- .cache/srtla-captures
cargo test --release -p packet-capture --test lab

RECEIVER=freewheel labs/srtla-capture/run-all.sh                                      # our receiver
FREEWHEEL_CAPTURES_DIR=$PWD/.cache/srtla-captures-freewheel-receiver \
  cargo test --release -p packet-capture --test lab

SENDER=freewheel labs/srtla-capture/run-all.sh                                        # our sender
FREEWHEEL_CAPTURES_DIR=$PWD/.cache/srtla-captures-freewheel-sender \
  cargo test --release -p packet-capture --test lab
```

Scenarios and topology: [`labs/srtla-capture/README.md`](../../../labs/srtla-capture/README.md).
