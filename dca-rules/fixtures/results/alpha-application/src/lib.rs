pub mod add_part {
    use dca_building_blocks::application::{ClosedResult, Refusal};
    use dca_building_blocks::hexagonal::port_in::UseCase;

    pub struct AddPartCommand {
        pub label: String,
    }

    /// The answer: added, or the expected refusal as a variant carrying a marked value.
    pub enum AddPartResult {
        Added(u64),
        LimitReached(PartLimitReached),
    }

    /// The refusal: the limit the part count reached.
    pub struct PartLimitReached {
        pub limit: u32,
    }

    impl Refusal for PartLimitReached {}

    impl ClosedResult for AddPartResult {
        fn refusal(&self) -> Option<&dyn Refusal> {
            match self {
                Self::Added(_) => None,
                Self::LimitReached(refused) => Some(refused),
            }
        }
    }

    pub trait AddPartInputPort: UseCase<AddPartCommand, Output = AddPartResult> {}
}

pub mod check_part {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    pub struct CheckPartCommand {
        pub part: u64,
    }

    /// A closed result returned by a method of the port's own - no variant carries a refusal
    /// (DCA-ERR-008).
    pub enum CheckPartResult {
        Accepted,
        Held,
    }

    pub trait CheckPartInputPort: InputPort {
        fn check(&self, command: CheckPartCommand) -> CheckPartResult;
    }
}

pub mod remove_part {
    use dca_building_blocks::hexagonal::port_in::UseCase;

    pub struct RemovePartCommand {
        pub part: u64,
    }

    pub struct RemovePartResult {
        pub part: u64,
    }

    pub trait RemovePartInputPort: UseCase<RemovePartCommand, Output = RemovePartResult> {}
}
