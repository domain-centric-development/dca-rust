pub mod incoming {
    use alpha_application::do_thing::Runner;
    use alpha_application::WrongRepository;
    use alpha_domain::Calculator;
    use alpha_infrastructure::Pool;
    use beta_api::events::Notice;

    use crate::outgoing::Store1;
}

pub mod outgoing {
    use alpha_application::other::OtherInputPort;

    pub struct Store1;
}
