use dca_building_blocks::hexagonal::port_out::{Repository, Store};
use kernel_domain::Ledger;

pub trait LedgerRepository: Repository<Ledger> {}

pub trait Archive: Store {}
