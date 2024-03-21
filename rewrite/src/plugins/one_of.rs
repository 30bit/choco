use crate::core::{Event, EventFlow, PluginError, StrRange};
use core::any::TypeId;

pub trait OneOf: Sized + 'static {
    fn one_of(param: &str) -> Option<Self>;

    fn prompt() -> &'static str;
}

pub struct Plugin<T: OneOf>(Option<T>);

impl<T: OneOf> Plugin<T> {
    pub fn last(&self) -> Option<&T> {
        self.0.as_ref()
    }
}

impl<T: OneOf> crate::core::Plugin for Plugin<T> {
    fn take_signal<P: crate::core::Plugin>(
        signal: StrRange,
        mut flow: EventFlow<P>,
    ) -> crate::core::PluginResult<Option<TypeId>> {
        if signal.substr() == T::prompt() {
            let err = || PluginError::new::<Self>(signal.range.clone());
            let param = flow.next().ok_or_else(|| err().with_msg("no param"))?;
            let raw = match param {
                Event::Raw(raw) if raw.is_signal() => raw,
                _ => return Err(err().with_msg("param is not a signal")),
            };
            let one_of = T::one_of(raw.as_of(flow.full_str()).substr());
            let Some(self_) = flow.plugins.get_sub_mut::<Self>() else {
                return Err(err().with_msg("can't find `Self` in `plugins`"));
            };
            self_.0 = one_of;
            Ok(Some(TypeId::of::<Self>()))
        } else {
            Ok(None)
        }
    }
}
