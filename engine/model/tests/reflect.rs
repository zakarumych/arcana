use std::{fmt::Debug, marker::PhantomData, rc::Rc};

use arcana_model::{ColorModel, ColorValue, Model, Reflect, Value, ValueError};
use athena::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};
use edict::entity::EntityId;
use gametime::TimeSpan;
use palette::{Hsv, Hsva, LinLuma, LinLumaa, Srgb, Srgba};
use smol_str::SmolStr;

fn round_trip<T: Reflect + PartialEq + Debug>(value: T, model: Model) -> Result<(), ValueError> {
    assert_eq!(T::model(), model);
    let encoded = value.to_value();
    assert_eq!(T::clone_from_value(&encoded)?, value);
    assert_eq!(T::from_value(encoded)?, value);
    Ok(())
}

#[test]
fn native_values_round_trip() -> Result<(), ValueError> {
    round_trip((), Model::Unit)?;
    round_trip(true, Model::Bool)?;
    round_trip(i64::MIN, Model::Int)?;
    round_trip(1.25_f64, Model::Float)?;
    round_trip(SmolStr::new("reflected value"), Model::String)?;
    round_trip(TimeSpan::new(1_234_567_890), Model::TimeSpan)?;
    round_trip(LinLuma::new(0.25), Model::Color(ColorModel::Luma))?;
    round_trip(LinLumaa::new(0.25, 0.5), Model::Color(ColorModel::Lumaa))?;
    round_trip(Srgb::new(0.25, 0.5, 0.75), Model::Color(ColorModel::Srgb))?;
    round_trip(
        Srgba::new(0.25, 0.5, 0.75, 0.5),
        Model::Color(ColorModel::Srgba),
    )?;
    round_trip(Hsv::new(210.0, 0.5, 0.75), Model::Color(ColorModel::Hsv))?;
    round_trip(
        Hsva::new(210.0, 0.5, 0.75, 0.5),
        Model::Color(ColorModel::Hsva),
    )?;
    round_trip(Vector2::new(1.0, 2.0), Model::Vec2)?;
    round_trip(Vector3::new(1.0, 2.0, 3.0), Model::Vec3)?;
    round_trip(Vector4::new(1.0, 2.0, 3.0, 4.0), Model::Vec4)?;
    round_trip(
        Matrix2::from_column_arrays([[1.0, 2.0], [3.0, 4.0]]),
        Model::Mat2,
    )?;
    round_trip(
        Matrix3::from_column_arrays([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]),
        Model::Mat3,
    )?;
    round_trip(
        Matrix4::from_column_arrays([
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ]),
        Model::Mat4,
    )
}

#[test]
fn nested_collections_round_trip() -> Result<(), ValueError> {
    let model = Model::Option(Some(Box::new(Model::Array {
        elem: Some(Box::new(Model::Option(Some(Box::new(Model::Int))))),
        len: None,
    })));
    round_trip(Some(vec![Some(7_i64), None, Some(-3)]), model.clone())?;
    round_trip(Some(Vec::<Option<i64>>::new()), model.clone())?;
    round_trip(None::<Vec<Option<i64>>>, model)
}

fn assert_error<T: Reflect>(value: Value, expected: &str) {
    let Err(borrowed_error) = T::clone_from_value(&value) else {
        panic!("Invalid borrowed value was accepted");
    };
    assert_eq!(borrowed_error.to_string(), expected);
    let Err(owned_error) = T::from_value(value) else {
        panic!("Invalid owned value was accepted");
    };
    assert_eq!(owned_error.to_string(), expected);
}

#[test]
fn errors_describe_expected_and_actual_kinds() {
    assert_error::<i64>(Value::Bool(true), "Expected Int, found Bool");
    assert_error::<Srgba>(
        Value::Color(ColorValue::Srgb(Srgb::new(0.25, 0.5, 0.75))),
        "Expected Color(Srgba), found Color(Srgb)",
    );
    assert_error::<Srgb>(Value::Unit, "Expected Color(Srgb), found Unit");
    assert_error::<Option<i64>>(Value::Int(1), "Expected Option, found Int");
    assert_error::<Vec<i64>>(Value::Option(None), "Expected Array, found Option");
    assert_error::<Vec<Option<i64>>>(
        Value::Array(vec![
            Value::Option(None),
            Value::Option(Some(Box::new(Value::Bool(false)))),
        ]),
        "Array element 1: Option value: Expected Int, found Bool",
    );
}

#[test]
fn default_apply_preserves_value_on_error() -> Result<(), ValueError> {
    let mut values = vec![Some(10_i64), None];
    let invalid = Value::Array(vec![
        Value::Option(Some(Box::new(Value::Int(20)))),
        Value::Option(Some(Box::new(Value::Bool(false)))),
    ]);
    assert!(values.apply(invalid).is_err());
    assert_eq!(values, vec![Some(10), None]);

    values.apply(vec![None, Some(30_i64)].to_value())?;
    assert_eq!(values, vec![None, Some(30)]);
    Ok(())
}

struct LocalValue<'a> {
    value: Rc<i64>,
    scope: PhantomData<&'a ()>,
}

impl Reflect for LocalValue<'_> {
    fn model() -> Model {
        Model::Int
    }

    fn to_value(&self) -> Value {
        self.value.as_ref().to_value()
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        let value = i64::from_value(value)?;
        if value < 0 {
            return Err(ValueError::Custom("Expected a nonnegative integer".into()));
        }
        Ok(LocalValue {
            value: Rc::new(value),
            scope: PhantomData,
        })
    }
}

fn scoped_value(_scope: &(), value: i64) -> LocalValue<'_> {
    LocalValue {
        value: Rc::new(value),
        scope: PhantomData,
    }
}

#[test]
fn reflection_supports_non_clone_local_types() -> Result<(), ValueError> {
    let scope = ();
    let mut value = scoped_value(&scope, 42);
    let stored = Rc::clone(&value.value);
    assert!(value.apply(Value::Int(-1)).is_err());
    assert!(Rc::ptr_eq(&value.value, &stored));
    value.apply(Value::Int(17))?;
    assert_eq!(*value.value, 17);

    let values = Some(vec![value]);
    let encoded = values.to_value();
    let reconstructed = Option::<Vec<LocalValue<'_>>>::clone_from_value(&encoded)?;
    assert_eq!(reconstructed.to_value(), encoded);
    Ok(())
}

#[test]
fn entity_references_preserve_bits_and_validate_the_domain() -> Result<(), ValueError> {
    let tag = arcana_intern::ident!(EntityId);
    let entity = EntityId::from_bits(u64::MAX).expect("fixture ID is nonzero");
    round_trip(entity, Model::Reference(tag))?;
    assert_eq!(entity.to_value(), Value::Reference(tag, u64::MAX));
    assert!(EntityId::from_value(Value::Reference(tag, 0)).is_err());
    assert!(EntityId::from_value(Value::Reference(arcana_intern::ident!(OtherId), 1)).is_err());
    assert!(EntityId::from_value(Value::Int(1)).is_err());
    Ok(())
}

#[test]
fn reference_defaults_preserve_the_unbound_reference_domain() {
    let model = Model::Reference(arcana_intern::ident!(EntityId));
    let value = model.default_value();
    assert_eq!(value, Value::Reference(arcana_intern::ident!(EntityId), 0));
    assert_eq!(value.model(), model);
    assert!(EntityId::from_value(value).is_err());
}
