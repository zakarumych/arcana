use crate::threaded::TAny;

/// Slot for storing a single value of `Any` type
/// with type-safe access, replacement and removal.
#[derive(Default)]
pub struct Slot(Option<TAny>);

impl From<Option<TAny>> for Slot {
    #[inline(always)]
    fn from(opt: Option<TAny>) -> Self {
        Slot(opt)
    }
}

impl From<TAny> for Slot {
    #[inline(always)]
    fn from(boxed: TAny) -> Self {
        Slot(Some(boxed))
    }
}

impl Slot {
    #[inline(always)]
    pub fn new() -> Self {
        Slot(None)
    }

    #[inline(always)]
    pub fn with_value<T>(value: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self(Some(TAny::new(value)))
    }

    #[inline(always)]
    pub fn into_inner(self) -> Option<TAny> {
        self.0
    }

    #[inline(always)]
    pub fn set<T>(&mut self, value: T)
    where
        T: Send + Sync + 'static,
    {
        if let Some(boxed) = &mut self.0 {
            if let Some(slot) = boxed.downcast_mut::<T>() {
                *slot = value;
                return;
            }
        }
        self.0 = Some(TAny::new(value));
    }

    #[inline(always)]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        if let Some(boxed) = &self.0 {
            return boxed.downcast_ref::<T>();
        }

        None
    }

    #[inline(always)]
    pub fn take<T: 'static>(&mut self) -> Option<T> {
        if let Some(tany) = &self.0 {
            if tany.is::<T>() {
                let tany = self.0.take().unwrap();
                let value = unsafe { tany.downcast::<T>().unwrap_unchecked() };
                return Some(value);
            }
        }

        None
    }

    #[inline(always)]
    pub fn clone_from<T>(&mut self, value: &T)
    where
        T: Clone + Send + Sync + 'static,
    {
        if let Some(boxed) = &mut self.0 {
            if let Some(slot) = boxed.downcast_mut::<T>() {
                slot.clone_from(value);
                return;
            }
        }
        self.0 = Some(TAny::new(value.clone()));
    }

    #[inline(always)]
    pub fn from_borrowed<T>(&mut self, value: &(impl ToOwned<Owned = T> + ?Sized))
    where
        T: Send + Sync + 'static,
    {
        if let Some(boxed) = &mut self.0 {
            if let Some(slot) = boxed.downcast_mut::<T>() {
                value.clone_into(slot);
                return;
            }
        }
        self.0 = Some(TAny::new(value.to_owned()));
    }
}
