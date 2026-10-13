//! A mission build that holds the game's loop up for longer than the
//! 5-second silence rule (slice NET): the client builds the mission inside
//! [`Client::receive_from`], and what the host sent meanwhile must count as
//! heard after the build, not before it. Fake time and in-memory sockets;
//! the keepalive thread is stood in for by Keepalive packets the build
//! hands the host once a second, as the thread would.

use super::tests::{build, spec};
use super::*;
use crate::host::{Host, HostConfig};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use tore_world::test_support::resources::resources;

const MS: Duration = Duration::from_millis(1);

type Queue = Rc<RefCell<VecDeque<(SocketAddr, Vec<u8>)>>>;

/// One end of an in-memory link: it reads its own queue and writes the
/// other end's, as `me`.
struct End {
    me: SocketAddr,
    inbox: Queue,
    outbox: Queue,
}

impl Datagrams for End {
    fn send_datagram(&mut self, _to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        self.outbox
            .borrow_mut()
            .push_back((self.me, datagram.to_vec()));
        Ok(())
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        Ok(self.inbox.borrow_mut().pop_front().map(|(from, datagram)| {
            let len = datagram.len().min(buf.len());
            buf[..len].copy_from_slice(&datagram[..len]);
            (len, from)
        }))
    }
}

/// A join whose lobby mission takes `stall` to build, the host running on
/// meanwhile: whether the client is still connected 2 seconds after the
/// build, and how many datagrams were left for the turn after it.
fn join_with_a_build_of(stall: Duration) -> (ClientPhase, usize) {
    let host_address: SocketAddr = "127.0.0.1:26900".parse().unwrap();
    let client_address: SocketAddr = "127.0.0.1:40000".parse().unwrap();
    let to_host: Queue = Rc::default();
    let to_client: Queue = Rc::default();
    let resources = Arc::new(resources());
    let host = Rc::new(RefCell::new(
        Host::new(
            spec(2, 2, 20),
            Arc::clone(&resources),
            HostConfig::new(build()),
        )
        .unwrap(),
    ));
    let mut host_end = End {
        me: host_address,
        inbox: Rc::clone(&to_host),
        outbox: Rc::clone(&to_client),
    };
    let mut client_end = End {
        me: client_address,
        inbox: Rc::clone(&to_client),
        outbox: Rc::clone(&to_host),
    };
    let clock = Rc::new(Cell::new(Duration::ZERO));
    let keepalive: Rc<RefCell<Option<Vec<u8>>>> = Rc::default();
    let mut client = Client::connect(
        ClientConfig::new(host_address, "Viper", build()),
        Arc::clone(&resources),
        Duration::ZERO,
    )
    .unwrap();
    // The build: the host runs on for `stall`, hearing a keepalive once a
    // second, and the world is built at the end of it.
    let builder_host = Rc::clone(&host);
    let builder_clock = Rc::clone(&clock);
    let builder_keepalive = Rc::clone(&keepalive);
    let builder_to_host = Rc::clone(&to_host);
    let builder_to_client = Rc::clone(&to_client);
    client.set_mission_builder(Box::new(move |spec, resources| {
        let start = builder_clock.get();
        let mut host = builder_host.borrow_mut();
        let mut end = End {
            me: host_address,
            inbox: Rc::clone(&builder_to_host),
            outbox: Rc::clone(&builder_to_client),
        };
        let mut last_keepalive = start;
        while builder_clock.get() < start + stall {
            let now = builder_clock.get() + 10 * MS;
            builder_clock.set(now);
            if now >= last_keepalive + Duration::from_secs(1)
                && let Some(datagram) = builder_keepalive.borrow().as_ref()
            {
                last_keepalive = now;
                builder_to_host
                    .borrow_mut()
                    .push_back((client_address, datagram.clone()));
            }
            host.receive_from(now, &mut end).unwrap();
            host.update(now);
            host.transmit(&mut end).unwrap();
        }
        World::new(spec, resources, Seating::Open)
    }));
    let mut built_at = None;
    let mut left = 0;
    while clock.get() < Duration::from_secs(30) {
        // The turn's time, read once at its start, as the bot and the game
        // do.
        let now = clock.get();
        {
            let mut host = host.borrow_mut();
            host.receive_from(now, &mut host_end).unwrap();
            host.update(now);
            host.transmit(&mut host_end).unwrap();
        }
        client.receive_from(now, &mut client_end).unwrap();
        if built_at.is_none() && client.last_build_time().is_some() {
            built_at = Some(clock.get());
            left = to_client.borrow().len();
        }
        client.update(now, &Controls::default());
        if keepalive.borrow().is_none() {
            *keepalive.borrow_mut() = client.keepalive_datagram();
        }
        client.transmit(&mut client_end).unwrap();
        if client.phase() == ClientPhase::Closed
            || built_at.is_some_and(|at| clock.get() >= at + Duration::from_secs(2))
        {
            break;
        }
        clock.set(clock.get() + 10 * MS);
    }
    assert!(built_at.is_some(), "the mission was never built");
    (client.phase(), left)
}

/// A 7-second build: the host's packets of those 7 seconds wait for the
/// next turn, and the client stays connected.
#[test]
fn a_build_longer_than_the_silence_rule_keeps_the_connection() {
    let (phase, left) = join_with_a_build_of(Duration::from_secs(7));
    assert!(
        left > 0,
        "the host's packets of the build were read with its stale time"
    );
    assert_ne!(phase, ClientPhase::Closed);
}

/// A quick build changes nothing: the client joins and flies.
#[test]
fn a_quick_build_joins_and_flies() {
    let (phase, _) = join_with_a_build_of(Duration::ZERO);
    assert_eq!(phase, ClientPhase::Flying);
}
