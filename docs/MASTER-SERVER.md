# Master server

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

How to build, configure, run and update `tore-master`, the program behind
the game's Internet Lobby. Design of 2026-10-05 for stages I and J of the
[multiplayer plan](multiplayer-plan.md#stages); **not built yet**, so the
commands below are what the slices will build. Every setting, default and
number is an *agent proposal* awaiting John's review unless it is credited
to him. How it works inside: [architecture](ARCHITECTURE.md#master-server-and-connectivity).
What it says on the wire: [master protocol](formats/master-protocol.md).

## Contents

- [What it is](#what-it-is)
- [What it needs](#what-it-needs)
- [Building it](#building-it)
- [Running it](#running-it)
- [The configuration file](#the-configuration-file)
- [Running it as a service](#running-it-as-a-service)
- [Ports and firewalls](#ports-and-firewalls)
- [The name](#the-name)
- [Logs and statistics](#logs-and-statistics)
- [What the master keeps](#what-the-master-keeps)
- [The relay's monthly allowance](#the-relays-monthly-allowance)
- [Updating](#updating)
- [Testing on one machine](#testing-on-one-machine)
- [Deploying at jroverton.com](#deploying-at-jrovertoncom)
- [When something is wrong](#when-something-is-wrong)
- [Security](#security)

## What it is

One small program that does five things for players on the internet:

- **Lists games.** A host that wants to be found sends its game's summary to
  the master every 30 seconds; the Internet Lobby screen asks the master for
  the list. A game that stops sending is dropped after 90 seconds.
- **Tests routers.** It tells a game how its router maps the game's port, so
  the game knows whether a direct path can work.
- **Introduces players to hosts,** so both can send to each other at the same
  moment and open a path through their routers (hole punching).
- **Relays** the traffic of a player and a host who cannot reach each other
  any other way, for example a player on a phone hotspot behind the carrier's
  shared address (CGNAT). Relaying is what costs transfer.
- **Counts anonymous statistics** sent by games that allow it.

It is not needed for playing on a local network or joining by address:
Direct Connection never talks to it. If the master is down, the Internet
Lobby is empty and everything else works.

It never reads Fighters Anthology's data. The machine that runs it needs no
import, and nothing derived from retail media ever reaches it.

## What it needs

- A Linux machine with a public IPv4 address, and ideally a public IPv6
  address, reachable on two UDP ports. John's decision of 2026-09-28: a
  Linode shared 2 GB plan with 1 TB of transfer a month.
- Very little else. One thread; well under one core even while relaying the
  plan's 64 channels at their full rate (at most about 8 MB/s in and out); a
  few tens of megabytes of memory with every table full.
- A name in DNS that the game is built to ask for
  ([the name](#the-name)).

It builds and runs on Windows and macOS too (the tests run there), but only
Linux with systemd is described here.

## Building it

From the repository root, on a machine with the project's Rust toolchain:

```sh
cargo build --release --locked -p tore-master
```

The program is `target/release/tore-master`, one file with no libraries
beside the system's C library. Build it where it will run, or on a system
with an older C library than the server's: a build from a rolling
distribution (such as the development machine's) can refuse to start on a
long-term release ("GLIBC_2.xx not found"). *Agent proposal:* the release
workflow also publishes a Linux `tore-master`, built on the same Ubuntu
runner as the game, which runs on any current long-term Ubuntu.

## Running it

```sh
tore-master --config /etc/tore-master/master.conf
tore-master --config master.conf --check-config
tore-master flood 203.0.113.10:26901 10
```

| Option | Meaning |
| --- | --- |
| `--config FILE` | The configuration file. Without one, every setting has its default |
| `--check-config` | Reads the configuration, says what it would do, and exits: 0 when it is good, 1 with the line and the reason when not |
| `flood TARGET SECONDS` | A load tool: sends every kind of request at a master from many ports of this machine for that long, then prints what came back. The master's limits should hold: no source answered with more bytes than it sent, and a normal request still answered during the flood. Only point it at a master you run |
| `--version` | The build |

It runs in the foreground, writes its lines to standard output, and stops
cleanly on Ctrl+C or SIGTERM: it tells every relayed pair that the relay is
closing and saves the month's relay figure.

## The configuration file

One setting per line, a name and a value; `#` starts a comment. An unknown
name, a value out of range, or a name given twice is refused with its line,
as the dedicated server's file is ([its rules](DEDICATED-SERVER.md#the-configuration-file)).

| Setting | Values | Default | Meaning |
| --- | --- | --- | --- |
| `listen` | `any` or one address | `any` | Where both ports listen; `any` is every IPv4 and IPv6 address |
| `port` | 1 to 65535 | 26901 | The main UDP port: listings, browsing, introductions, the relay, reports |
| `probe-port` | 1 to 65535, or 0 | 26902 | The second port of the router test; 0 turns the test off (games then never get the relay at once, only after the race) |
| `state-dir` | a folder | the configuration file's folder | Where the statistics, telemetry counts and the relay's monthly figure are written |
| `max-listings` | 1 to 100,000 | 2,000 | Listings at once |
| `listings-per-source` | 1 to 1,000 | 8 | Listings from one IPv4 address or IPv6 /64 network |
| `heartbeat` | 10 to 120 seconds | 30 | The interval the master tells hosts |
| `keep` | 5 to 60 seconds | 15 | How often hosts keep their router's mapping open |
| `expiry` | 30 to 600 seconds, at least twice `heartbeat` | 90 | A listing not heard from for this long is dropped |
| `browse-rate` | 1 to 1,000 a second | 20 | Browse and details requests answered per source |
| `introduce-rate` | 1 to 100 a second | 4 | Introductions per source |
| `answer-rate` | 100 to 100,000 a second | 5,000 | Answers of every kind together |
| `relay` | `on`, `off` | `on` | Whether to relay at all |
| `relay-channels` | 0 to 1,000 | 64 | Relayed pairs at once |
| `relay-channels-per-source` | 1 to 30 | 2 | Relayed pairs one player's address may have |
| `relay-rate` | 8 to 1,024 KB/s | 64 | Each channel's limit, each way |
| `relay-month-gb` | 0 to 100,000 | 800 | Relayed gigabytes sent out each calendar month (UTC) |
| `telemetry` | `on`, `off` | `on` | Whether to count the games' anonymous reports |
| `status-interval` | 0 to 3,600 seconds | 60 | How often the status line is written; 0 for never |

An example for the public master:

```text
# /etc/tore-master/master.conf
listen any
port 26901
probe-port 26902
state-dir /var/lib/tore-master
relay-month-gb 800
```

## Running it as a service

On Linux, a systemd unit, `/etc/systemd/system/tore-master.service`:

```ini
[Unit]
Description=T.O.R.E-Fighters master server
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/opt/tore-master/tore-master --config /etc/tore-master/master.conf
DynamicUser=yes
StateDirectory=tore-master
Restart=on-failure
RestartSec=5
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
MemoryMax=256M

[Install]
WantedBy=multi-user.target
```

Then `sudo systemctl daemon-reload` and `sudo systemctl enable --now
tore-master`. `DynamicUser` runs it as a user of its own that exists only
while it runs; `StateDirectory` gives that user `/var/lib/tore-master`; the
ports are above 1024, so it needs no privilege at all.

## Ports and firewalls

| Port | Protocol | From | Why |
| --- | --- | --- | --- |
| 26901 | UDP | anywhere, IPv4 and IPv6 | Everything the game says to the master, and the relay |
| 26902 | UDP | anywhere, IPv4 and IPv6 | The second half of the router test |
| 22 | TCP | the operator's address | SSH, to look after the machine |

On the machine, with ufw: `sudo ufw allow 26901:26902/udp`. On Linode, a
Cloud Firewall in front of the machine needs the same two UDP ports allowed
inbound; outbound stays open. The master needs no other port. The game's
own port, 26900, is not used on the master machine.

## The name

The game is built with the master's name and port (and a player or server
operator can change it in Options, `--master`, or the server's `master`
setting). *Agent proposal, for John to confirm:* `master.jroverton.com`,
port 26901, with an A record for the machine's IPv4 address and an AAAA
record for its IPv6 address. With both records, games that have IPv6 reach
it over IPv6 too, which lets them learn their own IPv6 address for direct
connections. A short time to live (300 seconds) while setting up makes a
move to another machine quick. Moving the master later means pointing the
name at the new machine: games ask the name again every 10 minutes and
whenever the master falls silent.

## Logs and statistics

- **Standard output** (the journal, under systemd: `journalctl -u
  tore-master`): the start lines (version, the sockets bound, the settings
  that differ from the defaults), one status line every `status-interval`,
  and notable events: a source over a limit (once a minute per source, with
  its address and the limit), a relay channel opened or closed (with the
  two ends' addresses and its bytes), the allowance reaching 95 and 100
  percent, configuration problems.

  ```text
  status listings=41 sources=318 browse/s=2.4 introductions/min=7 punched=5 relayed=2 channels=3 relay-month=12.7GB dropped(limit)=0 invalid=4
  ```

- **`state-dir/stats/YYYY-MM-DD.tsv`**: one line a minute with the same
  counts, for graphs. Kept 90 days, then deleted by the master.
- **`state-dir/telemetry/YYYY-MM-DD.tsv`**: the day's counts from the games'
  reports ([what the master keeps](#what-the-master-keeps)). Kept until
  deleted by hand.
- **`state-dir/relay-YYYY-MM.txt`**: the month's relayed bytes, written every
  minute so a restart does not forget them.

## What the master keeps

*Agent proposal; telemetry's defaults and contents are John's to decide.*

- **Listings** are kept in memory only, for as long as each host sends
  heartbeats. A master restart forgets them; every host lists itself again
  within one heartbeat.
- **Telemetry** is kept as counts per day: how many sessions, how long, how
  many humans, how players connected (local, by address, mapped port, IPv6,
  punched, relay) and how long it took, how routers map, which port-mapping
  method worked, relayed bytes, and, from stage K, host migrations. The
  number of distinct installs a day is counted with a salt drawn each day and
  never written down, so an install cannot be followed from one day to the
  next. No address is ever stored with telemetry.
- **Addresses** appear only in the journal's lines about limits and relay
  channels, for dealing with abuse. *Agent proposal:* keep the journal 14
  days on this machine (`MaxRetentionSec=14day` in
  `/etc/systemd/journald.conf`, which applies to the whole machine).
- The game's README says what is sent and how to turn it off.

## The relay's monthly allowance

Relaying is the only part that costs transfer. Every relayed byte arrives
and leaves once; Linode counts outgoing transfer against the plan (check
this on the account). The master counts what it relays out each calendar
month, UTC:

- At 95 percent of `relay-month-gb` it refuses new channels ("The Internet
  Lobby's relay is full for this month."); at 100 percent it closes the open
  ones. Players who can connect directly are not affected.
- The figure survives restarts (`relay-YYYY-MM.txt`) and starts again at
  zero on the first of the month.
- From the [plan's budget](multiplayer-plan.md#bandwidth-budget), a relayed
  player in a full mission costs about 60 MB an hour leaving the master, so
  800 GB is about 13,000 relayed player-hours a month, or 18 relayed
  players around the clock.
- Set it to about 80 percent of the plan's monthly transfer, less anything
  else the machine serves, so the plan is never exceeded.

## Updating

1. Build or download the new `tore-master`.
2. `tore-master --config /etc/tore-master/master.conf --check-config` with
   the new program.
3. `sudo systemctl stop tore-master`, replace
   `/opt/tore-master/tore-master`, `sudo systemctl start tore-master`.

Hosts list themselves again within one heartbeat (30 seconds) and browsers
refill. Relayed players are disconnected by the stop; stage K's rejoin
brings them back. The status line's `channels=` says how many are relayed
at the moment, so update when it is 0 if you can.

**Versions.** A master answers every master protocol version it knows, so
one master serves old and new games at once
([versions](formats/master-protocol.md#versions)). When a game release needs
a newer master protocol, update the master first, then release the game.

## Testing on one machine

Everything runs on one development machine without the public master, on
other ports than the public ones so nothing collides:

```text
# master.conf, beside the build
listen 127.0.0.1
port 26911
probe-port 26912
```

```sh
target/debug/tore-master --config master.conf
# A dedicated server that lists itself: in its configuration,
#   list on
#   master 127.0.0.1:26911
target/debug/tore-server --config server.conf --data-dir "$TORE_DATA_DIR"
# The games it lists, headless:
target/debug/tore-app --browse 5 --master 127.0.0.1:26911
# A bot that joins it through an introduction, and one through the relay:
target/debug/tore-bot --master 127.0.0.1:26911 --listing "Friday night" --data-dir "$TORE_DATA_DIR"
target/debug/tore-bot --master 127.0.0.1:26911 --listing "Friday night" --path relay --data-dir "$TORE_DATA_DIR"
# The flood tool against the local master:
target/debug/tore-master flood 127.0.0.1:26911 10
```

In the game, set the master's address in the Internet Lobby's Options (or
start it with `--master 127.0.0.1:26911`). On one machine every direct path
works, so the relay is forced with `--path relay`; routers, punching and
their failures are tested on the network simulator in the Rust tests. Port
mapping is tested against a fake router on loopback; trying it on a real
router changes that router, so it is a manual test.

## Deploying at jroverton.com

What John does, once the slices are built. The choices in the first step are
his ([open questions](MULTIPLAYER.md#open-questions)); the rest follows.

1. **Decide** the machine (a new Linode 2 GB, the agent proposal, or the
   existing jroverton.com server), its region (near most players: relayed
   traffic goes through it), the name (`master.jroverton.com` proposed) and
   the ports (UDP 26901 and 26902 proposed).
2. **Create the machine:** Ubuntu 24.04 LTS on the 2 GB shared plan, an SSH
   key, the region chosen. Linode gives it an IPv4 and an IPv6 address.
3. **DNS:** an A record and an AAAA record for the name, pointing at those
   addresses, time to live 300.
4. **Firewall:** a Linode Cloud Firewall allowing UDP 26901 and 26902 from
   anywhere and TCP 22 from home; on the machine `sudo ufw allow OpenSSH`,
   `sudo ufw allow 26901:26902/udp`, `sudo ufw enable`.
5. **Install:** copy `tore-master` to `/opt/tore-master/`, write
   `/etc/tore-master/master.conf` (the example above), install the unit
   ([as a service](#running-it-as-a-service)), `sudo systemctl enable --now
   tore-master`.
6. **Check:** `journalctl -u tore-master -f` shows the start lines and a
   status line a minute. From home: `tore-app --browse 5 --master
   master.jroverton.com:26901` answers (with no games yet), and `tore-master
   flood master.jroverton.com:26901 10` reports the limits held.
7. **Tell the lead** the name and port: the game's default master address is
   set to it, and from that build on the Internet Lobby uses it.
8. **Each month,** glance at the plan's transfer in the Linode Cloud Manager
   and the master's `relay-month=` figure.

## When something is wrong

| What a player sees | Look at |
| --- | --- |
| "The Internet Lobby does not answer" | Is the service running (`systemctl status tore-master`)? Does the name resolve to the machine (`dig master.jroverton.com A` and `AAAA`)? Are UDP 26901 and 26902 open in both firewalls? |
| A hosted game is not in the list | The host's game says why in its lobby's Messages. The host's machine may block the game's outgoing UDP, or the master refused it for a limit (the journal says which source) |
| Listed, but nobody can join | Most joins fall back to the relay; "relay is full" or "switched off" means the relay's settings or the allowance. If even the relay fails, the host's router dropped its mapping: the game keeps it open every 15 seconds, so this points at an unusual router |
| Joins work, but slowly | A relayed player's delay is the trip to the master and on to the host; a master far from both adds it twice |

## Security

- It runs as an unprivileged user of its own, on ports above 1024, writes
  only its `state-dir`, and reads nothing but its configuration file.
- It speaks only UDP, decodes nothing it cannot bound, never answers an
  unproven sender with more bytes than it sent, and limits every source and
  every kind of request ([master protocol](formats/master-protocol.md#security)).
  `tore-master flood` lets the operator see that for themselves.
- Traffic is not encrypted, as with the game's own: listing tokens and relay
  keys can be read by anyone who can watch the network between a game and
  the master. That lets such a person remove or change a listing, nothing
  more; the game's password and the host's checks are unchanged by the
  master.
- If the master is attacked or down, the Internet Lobby is empty and
  relayed players are disconnected. Local games, Direct Connection and
  joining by address work as before.
