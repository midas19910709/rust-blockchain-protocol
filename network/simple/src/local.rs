use std::collections::HashMap;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::{Arc, mpsc::{SyncSender, Receiver, sync_channel}};
use core::marker::PhantomData;
use core::hash::Hash;
use core::fmt::Debug;
use blockchain::chain::SharedBackend;
use blockchain::traits::{ChainQuery, ImportBlock};
use crate::{SimpleSync, SimpleSyncMessage, NetworkEnvironment, NetworkHandle, NetworkEvent, StatusProducer};

pub struct LocalNetwork<P, B, S> {
	senders: HashMap<P, SyncSender<(P, SimpleSyncMessage<B, S>)>>,
}

impl<P: Eq + Hash + Clone, B: Clone, S: Clone> LocalNetwork<P, B, S> {
	pub fn send(&self, peer: &P, message: (P, SimpleSyncMessage<B, S>)) {
		self.senders.get(peer).unwrap()
			.send(message).unwrap();
	}

	pub fn broadcast(&self, message: (P, SimpleSyncMessage<B, S>)) {
		for sender in self.senders.values() {
			sender.send(message.clone()).unwrap();
		}
	}
}

#[derive(Clone)]
pub struct LocalNetworkHandle<P, B, S> {
	peer_id: P,
	network: Arc<LocalNetwork<P, B, S>>
}

impl<P, B, S> NetworkEnvironment for LocalNetworkHandle<P, B, S> {
	type PeerId = P;
	type Message = SimpleSyncMessage<B, S>;
}

impl<P: Eq + Hash + Clone, B: Clone, S: Clone> NetworkHandle for LocalNetworkHandle<P, B, S> {
	fn send(&mut self, peer: &P, message: SimpleSyncMessage<B, S>) {
		self.network.send(peer, (self.peer_id.clone(), message));
	}

	fn broadcast(&mut self, message: SimpleSyncMessage<B, S>) {
		self.network.broadcast((self.peer_id.clone(), message));
	}
}

pub fn start_local_simple_peer<P, Ba, I, St>(
	mut handle: LocalNetworkHandle<P, Ba::Block, St::Status>,
	receiver: Receiver<(P, SimpleSyncMessage<Ba::Block, St::Status>)>,
	peer_id: P,
	backend: SharedBackend<Ba>,
	importer: I,
	status: St,
) -> JoinHandle<()> where
	P: Debug + Eq + Hash + Clone + Send + Sync + 'static,
	Ba: ChainQuery + Send + Sync + 'static,
	Ba::Block: Debug + Send + Sync,
	I: ImportBlock<Block=Ba::Block> + Send + Sync + 'static,
	St: StatusProducer + Send + Sync + 'static,
	St::Status: Clone + Debug + Send + Sync,
{
	thread::spawn(move || {
		let this_peer_id = peer_id.clone();

		let mut sync = SimpleSync {
			backend, importer, status,
			_marker: PhantomData
		};

		loop {
			for (peer_id, message) in receiver.try_iter() {
				println!("peer[{:?}] on message {:?}", this_peer_id, message);
				sync.on_message(&mut handle, &peer_id, message);
			}

			thread::sleep(Duration::from_millis(1000));
			println!("peer[{:?}] on tick", this_peer_id);
			sync.on_tick(&mut handle);
		}
	})
}

pub fn start_local_simple_sync<P, Ba, I, St>(
	peers: HashMap<P, (SharedBackend<Ba>, I, St)>
) where
	P: Debug + Eq + Hash + Clone + Send + Sync + 'static,
	Ba: ChainQuery + Send + Sync + 'static,
	Ba::Block: Debug + Send + Sync,
	I: ImportBlock<Block=Ba::Block> + Send + Sync + 'static,
	St: StatusProducer + Send + Sync + 'static,
	St::Status: Clone + Debug + Send + Sync,
{
	let mut senders: HashMap<P, SyncSender<(P, SimpleSyncMessage<Ba::Block, St::Status>)>> = HashMap::new();
	let mut peers_with_receivers: HashMap<P, (SharedBackend<Ba>, I, St, Receiver<(P, SimpleSyncMessage<Ba::Block, St::Status>)>)> = HashMap::new();
	for (peer_id, (backend, importer, status)) in peers {
		let (sender, receiver) = sync_channel(10);
		senders.insert(peer_id.clone(), sender);
		peers_with_receivers.insert(peer_id, (backend, importer, status, receiver));
	}

	let mut join_handles: Vec<JoinHandle<()>> = Vec::new();
	let network = Arc::new(LocalNetwork { senders });
	for (peer_id, (backend, importer, status, receiver)) in peers_with_receivers {
		let join_handle = start_local_simple_peer(
			LocalNetworkHandle {
				peer_id: peer_id.clone(),
				network: network.clone(),
			},
			receiver,
			peer_id,
			backend,
			importer,
			status,
		);
		join_handles.push(join_handle);
	}

	for join_handle in join_handles {
		join_handle.join().unwrap();
	}
}
