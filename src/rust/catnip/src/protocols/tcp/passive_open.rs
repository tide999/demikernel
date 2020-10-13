use bytes::Bytes;
use super::{
    constants::FALLBACK_MSS,
    established::state::{
        receiver::Receiver,
        sender::Sender,
        ControlBlock,
    },
    isn_generator::IsnGenerator,
};
use crate::{
    fail::Fail,
    protocols::{
        arp,
        ipv4,
        ipv4::datagram::{Ipv4Header2, Ipv4Protocol2},
        ethernet::frame::{EtherType2, Ethernet2Header2},
        tcp::{
            segment::{TcpSegment2, TcpHeader2, TcpOptions2},
            SeqNumber,
        },
    },
    runtime::Runtime,
    scheduler::SchedulerHandle,
};
use hashbrown::{HashMap, HashSet};
use std::{
    cell::RefCell,
    collections::{
        VecDeque,
    },
    convert::TryInto,
    future::Future,
    num::Wrapping,
    rc::Rc,
    task::{
        Context,
        Poll,
        Waker,
    },
    time::Duration,
};

struct InflightAccept {
    local_isn: SeqNumber,
    remote_isn: SeqNumber,
    window_size: u32,
    window_scale: u8,
    mss: usize,

    #[allow(unused)]
    handle: SchedulerHandle,
}

struct ReadySockets<RT: Runtime> {
    ready: VecDeque<Result<ControlBlock<RT>, Fail>>,
    endpoints: HashSet<ipv4::Endpoint>,
    waker: Option<Waker>,
}

impl<RT: Runtime> ReadySockets<RT> {
    fn push_ok(&mut self, cb: ControlBlock<RT>) {
        assert!(self.endpoints.insert(cb.remote));
        self.ready.push_back(Ok(cb));
        self.waker.take().map(|w| w.wake());
    }

    fn push_err(&mut self, err: Fail) {
        self.ready.push_back(Err(err));
        self.waker.take().map(|w| w.wake());
    }

    fn poll(&mut self, ctx: &mut Context) -> Poll<Result<ControlBlock<RT>, Fail>> {
        let r = match self.ready.pop_front() {
            Some(r) => r,
            None => {
                self.waker.replace(ctx.waker().clone());
                return Poll::Pending;
            },
        };
        if let Ok(ref cb) = r {
            assert!(self.endpoints.remove(&cb.remote));
        }
        Poll::Ready(r)
    }

    fn len(&self) -> usize {
        self.ready.len()
    }
}

pub struct PassiveSocket<RT: Runtime> {
    inflight: HashMap<ipv4::Endpoint, InflightAccept>,
    ready: Rc<RefCell<ReadySockets<RT>>>,

    max_backlog: usize,
    isn_generator: IsnGenerator,

    local: ipv4::Endpoint,
    rt: RT,
    arp: arp::Peer<RT>,
}

impl<RT: Runtime> PassiveSocket<RT> {
    pub fn new(local: ipv4::Endpoint, max_backlog: usize, rt: RT, arp: arp::Peer<RT>) -> Self {
        let ready = ReadySockets {
            ready: VecDeque::new(),
            endpoints: HashSet::new(),
            waker: None,
        };
        let ready = Rc::new(RefCell::new(ready));
        let nonce = rt.rng_gen();
        Self {
            inflight: HashMap::new(),
            ready,
            max_backlog,
            isn_generator: IsnGenerator::new(nonce),
            local,
            rt,
            arp,
        }
    }

    pub fn poll_accept(&mut self, ctx: &mut Context) -> Poll<Result<ControlBlock<RT>, Fail>> {
        self.ready.borrow_mut().poll(ctx)
    }

    pub fn receive2(&mut self, ip_header: &Ipv4Header2, header: &TcpHeader2) -> Result<(), Fail> {
        let remote = ipv4::Endpoint::new(ip_header.src_addr, header.src_port);
        if self.ready.borrow().endpoints.contains(&remote) {
            // TODO: What should we do if a packet shows up for a connection that hasn't been
            // `accept`ed yet?
            return Ok(());
        }
        let inflight_len = self.inflight.len();

        // If the packet is for an inflight connection, route it there.
        if self.inflight.contains_key(&remote) {
            if !header.ack {
                return Err(Fail::Malformed { details: "Expected ACK" });
            }
            // TODO: Add entry API.
            let &InflightAccept {
                local_isn,
                remote_isn,
                window_size,
                window_scale,
                mss,
                ..
            } = self.inflight.get(&remote).unwrap();
            if header.ack_num != local_isn + Wrapping(1) {
                return Err(Fail::Malformed { details: "Invalid SYN+ACK seq num" });
            }
            let sender = Sender::new(local_isn + Wrapping(1), window_size, window_scale, mss);
            let receiver = Receiver::new(
                remote_isn + Wrapping(1),
                self.rt.tcp_options().receive_window_size as u32,
            );
            self.inflight.remove(&remote);
            let cb = ControlBlock {
                local: self.local.clone(),
                remote: remote.clone(),
                rt: self.rt.clone(),
                arp: self.arp.clone(),
                sender,
                receiver,
            };
            self.ready.borrow_mut().push_ok(cb);
            return Ok(());
        }

        // Otherwise, start a new connection.
        if !header.syn || header.ack || header.rst {
            return Err(Fail::Malformed { details: "Invalid flags" });
        }
        if inflight_len + self.ready.borrow().len() >= self.max_backlog {
            // TODO: Should we send a RST here?
            return Err(Fail::ConnectionRefused {});
        }
        let local_isn = self.isn_generator.generate(&self.local, &remote);
        let remote_isn = header.seq_num;
        let future = Self::background(
            local_isn,
            remote_isn,
            self.local,
            remote.clone(),
            self.rt.clone(),
            self.arp.clone(),
            self.ready.clone(),
        );
        let handle = self.rt.spawn(future);

        let mut window_scale = 1;
        let mut mss = FALLBACK_MSS;
        for option in header.iter_options() {
            match option {
                TcpOptions2::WindowScale(w) => {
                    window_scale = *w;
                },
                TcpOptions2::MaximumSegmentSize(m) => {
                    mss = *m as usize;
                }
                _ => continue,
            }
        }
        let window_size = header
            .window_size
            .checked_shl(window_scale as u32)
            .expect("TODO: Window size overflow")
            .try_into()
            .expect("TODO: Window size overflow");
        let accept = InflightAccept {
            local_isn,
            remote_isn,
            window_size,
            window_scale,
            mss,
            handle,
        };
        self.inflight.insert(remote, accept);
        Ok(())
    }

    fn background(
        local_isn: SeqNumber,
        remote_isn: SeqNumber,
        local: ipv4::Endpoint,
        remote: ipv4::Endpoint,
        rt: RT,
        arp: arp::Peer<RT>,
        ready: Rc<RefCell<ReadySockets<RT>>>,
    ) -> impl Future<Output = ()> {
        let handshake_retries = 3usize;
        let handshake_timeout = Duration::from_secs(5);
        let max_window_size = 1024;

        async move {
            for _ in 0..handshake_retries {
                let remote_link_addr = match arp.query(remote.address()).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("ARP query failed: {:?}", e);
                        continue;
                    },
                };
                let mut tcp_hdr = TcpHeader2::new(local.port, remote.port);
                tcp_hdr.syn = true;
                tcp_hdr.seq_num = local_isn;
                tcp_hdr.ack = true;
                tcp_hdr.ack_num = remote_isn + Wrapping(1);
                tcp_hdr.window_size = max_window_size;

                let segment = TcpSegment2 {
                    ethernet2_hdr: Ethernet2Header2 {
                        dst_addr: remote_link_addr,
                        src_addr: rt.local_link_addr(),
                        ether_type: EtherType2::Ipv4,
                    },
                    ipv4_hdr: Ipv4Header2::new(local.addr, remote.addr, Ipv4Protocol2::Tcp),
                    tcp_hdr,
                    data: Bytes::new(),
                };
                rt.transmit2(segment);
                rt.wait(handshake_timeout).await;
            }
            ready.borrow_mut().push_err(Fail::Timeout {});
        }
    }
}