use crate::core::{EventFlow, PluginError, PluginResult, StrRange};
use core::any::TypeId;

use super::one_of::{peek_one_of, OneOf};

pub trait ManyOf: OneOf {
    fn append(&mut self, param: Self) -> bool;
}

pub struct Plugin<T: ManyOf>(Option<T>);

impl<T: ManyOf> Plugin<T> {
    pub fn last(&self) -> Option<&T> {
        self.0.as_ref()
    }
}

impl<T: ManyOf> crate::core::Plugin for Plugin<T> {
    fn take_signal<P: crate::core::Plugin>(
        signal: StrRange,
        mut flow: EventFlow<P>,
    ) -> PluginResult<Option<TypeId>> {
        if signal.substr() == T::prompt() {
            let err = || PluginError::new::<Self>(signal.range.clone());
            let one_of = peek_one_of(&mut flow, err);
            flow.next();
            let mut many_of: T = one_of?;
            while let Ok(one_of) = peek_one_of(&mut flow, err) {
                many_of.append(one_of);
                flow.next();
            }
            let Some(self_) = flow.plugins.get_sub_mut::<Self>() else {
                return Err(err().with_msg("can't find `Self` in `plugins`"));
            };
            self_.0 = Some(many_of);
            Ok(Some(TypeId::of::<Self>()))
        } else {
            Ok(None)
        }
    }
}
