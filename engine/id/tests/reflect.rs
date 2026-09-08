use std::num::NonZeroU64;

use arcana_id::{BaseId, Stid};
use arcana_model::{Model, Reflect, Value, ValueError};

#[test]
fn nonzero_ids_preserve_the_full_unsigned_range() -> Result<(), ValueError> {
    for bits in [NonZeroU64::MIN, NonZeroU64::MAX] {
        let base = BaseId::new(bits);
        assert_eq!(BaseId::from_value(base.to_value())?.get(), bits.get());
        let stid = Stid::new(bits);
        assert_eq!(Stid::from_value(stid.to_value())?, stid);
        assert_eq!(
            stid.to_value(),
            Value::Reference(arcana_intern::ident!(Stid), bits.get())
        );
    }
    assert_eq!(
        BaseId::model(),
        Model::Reference(arcana_intern::ident!(BaseId))
    );
    Ok(())
}

#[test]
fn nonzero_ids_reject_unbound_and_wrong_domain_references() {
    assert!(BaseId::from_value(BaseId::model().default_value()).is_err());
    assert!(Stid::from_value(Stid::model().default_value()).is_err());
    assert!(BaseId::from_value(Stid::new(NonZeroU64::MIN).to_value()).is_err());
    assert!(Stid::from_value(BaseId::new(NonZeroU64::MIN).to_value()).is_err());
    assert!(Stid::from_value(Value::Int(1)).is_err());
}
