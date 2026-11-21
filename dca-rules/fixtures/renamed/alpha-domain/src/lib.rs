use dca_building_blocks::ddd::tactical::{AggregateRoot, Entity, Factory, Id, Specification, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThingId(u128);
impl Value for ThingId {}
impl Id for ThingId {}

/// An aggregate root under the configured suffix.
pub struct Thing {
    id: ThingId,
}
impl Entity for Thing {
    type Id = ThingId;
    fn id(&self) -> &ThingId {
        &self.id
    }
}
impl AggregateRoot for Thing {}

/// DCA-TAC-001: named by the configured aggregate-root suffix, without the role.
pub struct LedgerRoot;

/// DCA-ADV-013: a factory not named by the configured suffix; `ThingMaker` is.
pub struct ThingFactory;
impl Factory for ThingFactory {}
pub struct ThingMaker;
impl Factory for ThingMaker {}

/// DCA-ADV-017 selects by the configured suffix too: this one sits in the domain and passes.
pub struct OpenRule;
impl Specification<Thing> for OpenRule {
    fn is_satisfied_by(&self, _thing: &Thing) -> bool {
        true
    }
}
