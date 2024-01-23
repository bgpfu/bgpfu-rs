use std::fmt::Debug;

use crate::Error;

use super::Operation;

#[derive(Debug, Clone)]
pub(super) struct Required<T> {
    value: Option<T>,
}

impl<T> Required<T> {
    pub(super) const fn init() -> Self {
        Self { value: None }
    }

    pub(super) fn set(&mut self, value: T) {
        self.value = Some(value);
    }

    pub(super) fn require<O>(self, param_name: &'static str) -> Result<T, Error>
    where
        O: Operation,
    {
        self.value
            .ok_or_else(|| Error::missing_operation_parameter(O::NAME, param_name))
    }
}
