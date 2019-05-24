//! Chain importer and block builder.

mod action;

pub use self::action::{SharedBackend, ImportAction};

use crate::traits::{BlockImporter, RawImporter, ImportOperation};
use std::sync::{Arc, Mutex};
use std::{fmt, error as stderror};

/// Error type for chain.
#[derive(Debug)]
pub enum Error {
	/// Backend error.
	Backend(Box<stderror::Error>),
	/// Executor error.
	Executor(Box<stderror::Error>),
	/// Block is genesis block and cannot be imported.
	IsGenesis,
	/// Parent is not in the backend so block cannot be imported.
	ParentNotFound,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl stderror::Error for Error {
	fn source(&self) -> Option<&(dyn stderror::Error + 'static)> {
		match self {
			Error::Backend(e) => Some(e.as_ref()),
			Error::Executor(e) => Some(e.as_ref()),
			Error::IsGenesis | Error::ParentNotFound => None,
		}
	}
}

/// An importer that can be shared across threads.
pub struct SharedImporter<I> {
	importer: Arc<Mutex<I>>,
}

impl<I> SharedImporter<I> {
	/// Create a new shared import block.
	pub fn new(importer: I) -> Self {
		Self {
			importer: Arc::new(Mutex::new(importer)),
		}
	}
}

impl<I> Clone for SharedImporter<I> {
	fn clone(&self) -> Self {
		Self {
			importer: self.importer.clone(),
		}
	}
}

impl<I: BlockImporter> BlockImporter for SharedImporter<I> {
	type Block = I::Block;
	type Error = I::Error;

	fn import_block(&mut self, block: Self::Block) -> Result<(), Self::Error> {
		self.importer.lock().expect("Lock is poisoned")
			.import_block(block)
	}
}

impl<I: RawImporter> RawImporter for SharedImporter<I> {
	type Block = I::Block;
	type State = I::State;
	type Error = I::Error;

	fn import_raw(
		&mut self,
		raw: ImportOperation<Self::Block, Self::State>
	) -> Result<(), Self::Error> {
		self.importer.lock().expect("Lock is poisoned")
			.import_raw(raw)
	}
}
