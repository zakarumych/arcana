use arcana_id::Stid;
use arcana_intern::Name;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EventNodeDesc {
    pub outflow: Name,
    pub outputs: Vec<(Name, Stid)>,
}
