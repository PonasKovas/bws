use super::{SOption, SStr, SString, SUnit};
use std::{
    any::{Any, TypeId},
    backtrace::{Backtrace, BacktraceStatus},
    panic::UnwindSafe,
};

#[repr(C)]
pub enum MaybePanicked<T = SUnit> {
    Ok(T),
    Panic(SOption<PanicMessage>),
}

#[repr(C)]
pub enum PanicMessage {
    Ref(SStr<'static>),
    Owned(SString),
}

impl<T> MaybePanicked<T> {
    pub fn new<R: Into<T>, F: FnOnce() -> R + UnwindSafe>(f: F) -> Self {
        match std::panic::catch_unwind(f) {
            Ok(result) => Self::Ok(result.into()),
            Err(payload) => {
                let message = if payload.is::<String>() {
                    SOption::Some(PanicMessage::Owned(
                        (*payload.downcast::<String>().unwrap()).into(),
                    ))
                } else if payload.is::<&'static str>() {
                    SOption::Some(PanicMessage::Ref(
                        (*payload.downcast::<&'static str>().unwrap()).into(),
                    ))
                } else {
                    SOption::None
                };

                Self::Panic(message)
            }
        }
    }
    pub fn unwrap(self) -> T {
        match self {
            Self::Ok(result) => result,
            Self::Panic(message) => {
                // if BWS_SHOW_ALL_BACKTRACES set to anything other than 0 or false
                if std::env::var_os("BWS_SHOW_ALL_BACKTRACES")
                    .map_or(false, |s| s != "0" && s != "false")
                {
                    let bt = Backtrace::force_capture();
                    if bt.status() == BacktraceStatus::Captured {
                        eprintln!("{}", bt);
                    }
                }

                std::panic::resume_unwind(match message {
                    SOption::Some(PanicMessage::Ref(s)) => Box::new(s.into_str()),
                    SOption::Some(PanicMessage::Owned(s)) => Box::new(s.into_string()),
                    // only &'static str and String are supported for now
                    SOption::None => Box::new("<unknown payload type>"),
                })
            }
        }
    }
}
