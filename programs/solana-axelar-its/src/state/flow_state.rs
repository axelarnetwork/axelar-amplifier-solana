use crate::errors::ItsError;
use anchor_lang::prelude::*;
use std::time::Duration;

const EPOCH_TIME: Duration = Duration::from_secs(6 * 60 * 60); // 6 hours

#[derive(Debug, Clone, Copy)]
pub(crate) enum FlowDirection {
    In,
    Out,
}

/// Struct containing flow information for a specific epoch.
#[derive(Clone, Debug, Eq, PartialEq, InitSpace, AnchorSerialize, AnchorDeserialize)]
pub struct FlowState {
    pub flow_limit: Option<u64>,
    pub flow_in: u64,
    pub flow_out: u64,
    pub epoch: u64,
}

impl FlowState {
    pub(crate) const fn new(flow_limit: Option<u64>, epoch: u64) -> Self {
        Self {
            flow_in: 0,
            flow_out: 0,
            epoch,
            flow_limit,
        }
    }

    pub(crate) fn add_flow(&mut self, amount: u64, direction: FlowDirection) -> Result<()> {
        let Some(flow_limit) = self.flow_limit else {
            return Ok(());
        };

        let (to_add, to_compare) = match direction {
            FlowDirection::In => (&mut self.flow_in, self.flow_out),
            FlowDirection::Out => (&mut self.flow_out, self.flow_in),
        };

        Self::update_flow(flow_limit, to_add, to_compare, amount)
    }

    fn update_flow(flow_limit: u64, to_add: &mut u64, to_compare: u64, amount: u64) -> Result<()> {
        // Individual transfer amount cannot exceed the flow limit
        if amount > flow_limit {
            msg!("Flow limit exceeded");
            return err!(ItsError::InvalidArgument);
        }

        // Calculate new flow amount after adding the transfer
        let new_flow = to_add
            .checked_add(amount)
            .ok_or(ItsError::ArithmeticOverflow)?;

        // Calculate net flow: |new_flow - to_compare|
        // The flow limit is interpreted as a limit over the net amount of tokens
        // transferred from one chain to another within a six hours time window.
        let net_flow = if new_flow >= to_compare {
            new_flow - to_compare
        } else {
            to_compare - new_flow
        };

        // Check if net flow exceeds the limit
        if net_flow > flow_limit {
            msg!("Flow limit exceeded");
            return err!(ItsError::InvalidArgument);
        }

        *to_add = new_flow;

        Ok(())
    }
}

pub fn current_flow_epoch() -> Result<u64> {
    flow_epoch_with_timestamp(Clock::get()?.unix_timestamp)
}

/// Returns the current flow epoch based on the provided clock.
///
/// # Errors
///
/// Returns an error if conversion from clock to internal flow epoch fails.
pub fn flow_epoch_with_timestamp(timestamp: i64) -> Result<u64> {
    let unix_timestamp: u64 = timestamp
        .try_into()
        .map_err(|_err| ItsError::ArithmeticOverflow)?;

    unix_timestamp
        .checked_div(EPOCH_TIME.as_secs())
        .ok_or_else(|| ItsError::ArithmeticOverflow.into())
}
