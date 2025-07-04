use std::{backtrace::Backtrace, fmt, mem::ManuallyDrop, sync::Arc};

/// Universal error type for the Arcana engine.
///
/// Unhandled errors can be wrapped to this type
/// and returned back to the caller.
///
/// The engine will handle this error by logging it,
/// reporting to the user appropriately and
/// continue execution if possible.
pub struct Error {
    /// Inner error is boxed to reduce size of the `Error` type.
    /// This helps happy path execution to be more efficient.
    inner: Box<ErrorInner>,
}

impl Error {
    /// Wraps an error into `Error`.
    ///
    /// Returns argument if it is already an `Error`.
    pub fn wrap<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        // Attempt to cast error.
        // It may be an `Error` already.
        match cast_same(error) {
            Ok(error) => error,
            Err(error) => Error {
                inner: Box::new(ErrorInner {
                    source: Arc::new(error),
                    backtrace: Backtrace::capture(),
                }),
            },
        }
    }

    /// Creates a new `Error` with a static message.
    pub fn msg(msg: String) -> Self {
        struct MsgError(String);

        impl std::error::Error for MsgError {}

        impl fmt::Display for MsgError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for MsgError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        Error {
            inner: Box::new(ErrorInner {
                source: Arc::new(MsgError(msg)),
                backtrace: Backtrace::capture(),
            }),
        }
    }

    #[doc(hidden)]
    pub fn from_arc_error(source: Arc<dyn std::error::Error + Send + Sync>) -> Self {
        Error {
            inner: Box::new(ErrorInner {
                source,
                backtrace: Backtrace::capture(),
            }),
        }
    }

    /// Returns the type ID of the inner error.
    pub fn backtrace(&self) -> &Backtrace {
        &self.inner.backtrace
    }

    /// Returns a reference to the inner error.
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: std::error::Error + 'static,
    {
        self.inner.source.downcast_ref::<E>()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Error: {:?}\nBacktrace: {:?}",
            self.inner.source, self.inner.backtrace
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.inner.source, f)
    }
}

impl std::error::Error for Error {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        Some(&*self.inner.source)
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.inner.source)
    }
}

struct ErrorInner {
    source: Arc<dyn std::error::Error + Send + Sync>,
    backtrace: Backtrace,
}

pub trait UnifyError<T, E> {
    /// Converts `Result<T, E>` to `Result<T, Error>`.
    fn unify_error(self) -> Result<T, Error>;
}

impl<T, E> UnifyError<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn unify_error(self) -> Result<T, Error> {
        match self {
            Ok(value) => Ok(value),
            Err(e) => Err(Error::wrap(e)),
        }
    }
}

struct ErrorContext<C> {
    source: Arc<dyn std::error::Error + Send + Sync>,
    context: C,
}

impl<C> fmt::Debug for ErrorContext<C>
where
    C: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.source, self.context)
    }
}

impl<C> fmt::Display for ErrorContext<C>
where
    C: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.source, self.context)
    }
}

impl<C> std::error::Error for ErrorContext<C>
where
    C: fmt::Display,
{
    fn cause(&self) -> Option<&dyn std::error::Error> {
        Some(&*self.source)
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.source)
    }
}

pub trait WithContext<T> {
    /// Adds context to the error.
    fn with_context<C>(self, context: C) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static;
}

impl<T, E> WithContext<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[inline]
    fn with_context<C>(self, context: C) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        match self {
            Ok(value) => Ok(value),
            Err(e) => Err(error_with_context(Error::wrap(e), context)),
        }
    }
}

fn cast_same<A: 'static, B: 'static>(a: A) -> Result<B, A> {
    if std::any::TypeId::of::<A>() == std::any::TypeId::of::<B>() {
        let mut md = std::mem::ManuallyDrop::new(a);

        // SAFETY: The A is guaranteed to be of type B here.
        // Thus cast is between same type.
        let md = unsafe { &mut *(&raw mut md as *mut ManuallyDrop<B>) };

        // SAFETY: ManuallyDrop is not used after this point, so taking it is safe.
        Ok(unsafe { ManuallyDrop::take(md) })
    } else {
        Err(a)
    }
}

// Optimize for the happy path.
#[cold]
fn error_with_context<C>(mut error: Error, context: C) -> Error
where
    C: fmt::Display + Send + Sync + 'static,
{
    struct DummyError;

    impl fmt::Debug for DummyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("DummyError")
        }
    }

    impl fmt::Display for DummyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("DummyError")
        }
    }

    impl std::error::Error for DummyError {}

    let source = std::mem::replace(&mut error.inner.source, Arc::new(DummyError));
    error.inner.source = Arc::new(ErrorContext { source, context });
    error
}

#[doc(hidden)]
pub mod for_macro {
    use std::fmt;

    pub use std::format;

    use crate::Error;

    struct ClosureError<F>(pub F);

    impl<F> fmt::Debug for ClosureError<F>
    where
        F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            (self.0)(f)
        }
    }

    impl<F> fmt::Display for ClosureError<F>
    where
        F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            (self.0)(f)
        }
    }

    impl<F> std::error::Error for ClosureError<F> where F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result {}

    pub fn closure_error<F>(f: F) -> Error
    where
        F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result + Send + Sync + 'static,
    {
        Error::wrap(ClosureError(f))
    }
}

#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $args:expr)* $(,)?) => {
        $crate::for_macro::closure_error(move |f| write!(f, $fmt $(, $args)*))
    };
}

#[macro_export]
macro_rules! msg_error {
    ($fmt:literal $(, $args:expr)* $(,)?) => {
        $crate::Error::msg(format!($fmt $(, $args)*))
    };
}

#[macro_export]
macro_rules! fail {
    ($fmt:literal $(, $args:expr)* $(,)?) => {
        return Err($crate::for_macro::closure_error(move |f| write!(f, $fmt $(, $args)*)))
    };
}
