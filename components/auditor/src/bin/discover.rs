// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use async_std::task;
use libp2p::mdns::service::{build_query_response, MdnsPacket, MdnsService};
use libp2p::{identity, PeerId};
use std::time::Duration;
use std::{
  error::Error,
  task::{Context, Poll},
};

fn main() -> Result<(), Box<dyn Error>> {
  // Creating an identity Keypair for the local node, obtaining the local PeerId from the PublicKey.
  // Create a random PeerId
  let local_key = identity::Keypair::generate_ed25519();
  let local_peer_id = PeerId::from(local_key.public());
  println!("Local peer id: {:?}", local_peer_id);

  // This example provides passive discovery of the libp2p nodes on the
  // network that send mDNS queries and answers.
  task::block_on(async move {
    let mut service = MdnsService::new()?;
    loop {
      let (mut srv, packet) = service.next().await;
      match packet {
        MdnsPacket::Query(query) => {
          // We detected a libp2p mDNS query on the network. In a real application, you
          // probably want to answer this query by doing `query.respond(...)`.
          println!("Detected query from {:?}", query.remote_addr());
          let resp = build_query_response(query.query_id(),
                                          local_peer_id.clone(),
                                          vec![].into_iter(),
                                          Duration::from_secs(5)).unwrap();
          srv.enqueue_response(resp);
        }
        MdnsPacket::Response(response) => {
          // We detected a libp2p mDNS response on the network. Responses are for
          // everyone and not just for the requester, which makes it possible to
          // passively listen.
          for peer in response.discovered_peers() {
            if peer.id() != &local_peer_id {
              println!("Discovered peer {:?}", peer.id());
              // These are the self-reported addresses of the peer we just discovered.
              for addr in peer.addresses() {
                println!("\tAddress = {:?}", addr);
              }
            }
          }
        }
        MdnsPacket::ServiceDiscovery(query) => {
          // The last possibility is a service detection query from DNS-SD.
          // Just like `Query`, in a real application you probably want to call
          // `query.respond`.
          println!("Detected service query from {:?}", query.remote_addr());
        }
      }
      service = srv
    }
  })
}
