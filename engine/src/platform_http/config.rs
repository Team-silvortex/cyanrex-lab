//! Shared trusted configuration rules, not admission allocation or resource ownership.
use super::{security, TaskContentHttpConfigError};

pub(super) fn validate(
    origin: &str,
    capacities: &[usize],
) -> Result<(), TaskContentHttpConfigError> {
    security::validate_origin(origin)?;
    if capacities
        .iter()
        .any(|capacity| !(1..=16).contains(capacity))
    {
        return Err(TaskContentHttpConfigError::InvalidCapacity);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
