use dca_building_blocks::ddd::tactical::{Entity, Id, Timestamp, Value};

use crate::member::MemberId;

/// A loan of a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LoanId(u128);

impl LoanId {
    /// The identity from its 128 bits.
    pub const fn from_u128(bits: u128) -> LoanId {
        LoanId(bits)
    }

    /// The 128 bits.
    pub const fn as_u128(&self) -> u128 {
        self.0
    }
}

impl Value for LoanId {}
impl Id for LoanId {}

/// A loan, a state machine: active until the copy comes back, then returned. Each state wraps a
/// struct with private fields, so only this module builds one and moves it on.
#[derive(Debug, Clone)]
pub enum Loan {
    Active(ActiveLoan),
    Returned(ReturnedLoan),
}

/// A loan running.
#[derive(Debug, Clone)]
pub struct ActiveLoan {
    id: LoanId,
    member: MemberId,
    due: Timestamp,
}

/// A loan ended.
#[derive(Debug, Clone)]
pub struct ReturnedLoan {
    id: LoanId,
    member: MemberId,
    returned_on: Timestamp,
}

impl Loan {
    pub(crate) fn begin(id: LoanId, member: MemberId, due: Timestamp) -> Loan {
        Loan::Active(ActiveLoan { id, member, due })
    }

    /// Who borrowed the copy.
    pub fn member(&self) -> &MemberId {
        match self {
            Loan::Active(loan) => &loan.member,
            Loan::Returned(loan) => &loan.member,
        }
    }

    /// Whether the copy is still out.
    pub fn is_active(&self) -> bool {
        matches!(self, Loan::Active(_))
    }

    /// The loan, ended now; a returned loan stays as it was.
    pub(crate) fn returned(self, at: Timestamp) -> Loan {
        match self {
            Loan::Active(loan) => Loan::Returned(ReturnedLoan {
                id: loan.id,
                member: loan.member,
                returned_on: at,
            }),
            returned @ Loan::Returned(_) => returned,
        }
    }
}

impl ActiveLoan {
    /// When the copy is due back.
    pub fn due(&self) -> Timestamp {
        self.due
    }
}

impl ReturnedLoan {
    /// When the copy came back.
    pub fn returned_on(&self) -> Timestamp {
        self.returned_on
    }
}

impl Entity for Loan {
    type Id = LoanId;
    fn id(&self) -> &LoanId {
        match self {
            Loan::Active(loan) => &loan.id,
            Loan::Returned(loan) => &loan.id,
        }
    }
}
