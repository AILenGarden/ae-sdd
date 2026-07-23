use crate::CounterError;

macro_rules! monotonic_counter {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub const ZERO: Self = Self(0);

            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }

            pub fn checked_next(self) -> Result<Self, CounterError> {
                self.0
                    .checked_add(1)
                    .map(Self)
                    .ok_or(CounterError::Overflow {
                        counter: stringify!($name),
                        current: self.0,
                    })
            }

            pub fn advance_to(self, next: Self) -> Result<Self, CounterError> {
                if next > self {
                    Ok(next)
                } else {
                    Err(CounterError::NotMonotonic {
                        counter: stringify!($name),
                        current: self.0,
                        next: next.0,
                    })
                }
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

monotonic_counter!(StateRevision);
monotonic_counter!(FencingToken);
monotonic_counter!(InventoryGeneration);
monotonic_counter!(ContextRevision);
monotonic_counter!(EventSequence);
monotonic_counter!(TurnSequence);
monotonic_counter!(ContextGeneration);
monotonic_counter!(SampleSequence);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_counter_rejects_equal_or_lower_values() {
        let current = StateRevision::new(7);

        assert_eq!(
            current.advance_to(StateRevision::new(8)),
            Ok(StateRevision::new(8))
        );
        assert!(current.advance_to(StateRevision::new(7)).is_err());
        assert!(current.advance_to(StateRevision::new(6)).is_err());
    }

    #[test]
    fn monotonic_counter_reports_overflow() {
        assert_eq!(
            FencingToken::new(u64::MAX).checked_next(),
            Err(CounterError::Overflow {
                counter: "FencingToken",
                current: u64::MAX,
            })
        );
    }
}
