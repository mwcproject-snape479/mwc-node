// Copyright 2016 The Grin Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate grin_p2p as p2p;
extern crate mioco;
extern crate env_logger;

use std::io;
use std::sync::Arc;
use std::time;

#[test]
fn peer_handshake() {
  env_logger::init().unwrap();

  mioco::start(|| -> io::Result<()> {
    let server = Arc::new(p2p::Server::new());
    let in_server = server.clone();
		mioco::spawn(move || -> io::Result<()> {
      try!(in_server.start());
			Ok(())
		});

    // given server a little time to start
    mioco::sleep(time::Duration::from_millis(200));

    let addr =  p2p::DEFAULT_LISTEN_ADDR.parse().unwrap();
    let peer = try!(p2p::Server::connect_as_client(addr).map_err(|_| io::Error::last_os_error()));
    let peer = Arc::new(peer);
    let in_peer = peer.clone();
		mioco::spawn(move || -> io::Result<()> {
      in_peer.run(&p2p::DummyAdapter{});
      Ok(())
    });
    mioco::sleep(time::Duration::from_millis(100));
    peer.send_ping();
    mioco::sleep(time::Duration::from_millis(100));
    assert!(peer.sent_bytes() > 0);

    server.stop();
    Ok(())
  }).unwrap().unwrap();
}
