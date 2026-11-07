pub mod incoming {
    /// Matches over the variants through the enum's path; names no failure.
    pub mod page {
        use alpha_application::add_part::{AddPartCommand, AddPartInputPort, AddPartResult};

        pub fn add(port: &dyn AddPartInputPort, label: String) -> u16 {
            match port.execute(AddPartCommand { label }) {
                AddPartResult::Added(_) => 201,
                AddPartResult::LimitReached(_) => 409,
            }
        }
    }

    /// Matches over variants it imported by name.
    pub mod web {
        use alpha_application::check_part::CheckPartResult::{Accepted, Held};
        use alpha_application::check_part::{CheckPartCommand, CheckPartInputPort};

        pub fn check(port: &dyn CheckPartInputPort, part: u64) -> u16 {
            match port.check(CheckPartCommand { part }) {
                Accepted => 200,
                Held => 423,
            }
        }
    }

    /// Drives a use case and names neither a failure nor a variant of a closed result.
    pub mod rest {
        use alpha_application::remove_part::{RemovePartCommand, RemovePartInputPort};

        pub fn remove(port: &dyn RemovePartInputPort, part: u64) -> u64 {
            port.execute(RemovePartCommand { part }).part
        }
    }

    /// Holds the closed result and looks at none of its variants.
    pub mod feed {
        use alpha_application::add_part::{AddPartCommand, AddPartInputPort, AddPartResult};

        pub fn on(port: &dyn AddPartInputPort, label: String) {
            let _ignored: AddPartResult = port.execute(AddPartCommand { label });
        }
    }
}
