pub mod common {
    use alpha_domain::Thing;
    use dca_building_blocks::hexagonal::port_out::Repository;

    /// A repository named by the configured suffix: DCA-NAM-004 and DCA-TAC-016 pass it.
    pub trait ThingRepo: Repository<Thing> {}

    /// DCA-NAM-004: names a repository by the configured suffix but does not end with it (and, no output port, DCA-HEX-009).
    pub trait ThingRepoIndex {}

    /// DCA-HEX-009: a trait in the configured shared module that is no output port.
    pub trait Clock {}
}

pub mod add_part {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    /// An input port named by the configured suffix.
    pub trait AddPartPort: InputPort {}

    /// A use case named by the configured suffix.
    pub struct AddPartService;
    impl AddPartPort for AddPartService {}
}

pub mod rename_part {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    /// DCA-NAM-003: not named by the configured input-port suffix.
    pub trait RenamePartContract: InputPort {}

    /// DCA-NAM-001: not named by the configured use-case suffix.
    pub struct RenamePartUseCase;
    impl RenamePartContract for RenamePartUseCase {}
}
