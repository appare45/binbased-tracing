mod trampoline;
mod plan;

use plan::plan_instrumentation;

use std::sync::Arc;

use crate::{
    error::InstrumentError,
    event::TargetId,
    event_buffer::EventBuffer,
    proc,
    ptrace,
};

pub use crate::symbol_analyzer::FunctionAnalysis;

pub struct Instrumenter {
    pub proc: proc::Proc,
    runtime_offsets: Option<crate::dwarf::RuntimeOffsets>,
}

impl Instrumenter {
    pub fn new(mut proc: proc::Proc) -> Result<Self, InstrumentError> {
        proc.init_regions().ok_or(InstrumentError::ProcError(
            crate::error::ProcError::IoError(std::io::Error::other("failed to read /proc/maps")),
        ))?;
        Ok(Self {
            proc,
            runtime_offsets: None,
        })
    }

    pub fn add_target(mut self, analysis: &FunctionAnalysis, buffer: Arc<EventBuffer>, target_id: TargetId) -> Result<Self, InstrumentError> {
        let plan = plan_instrumentation(
            &mut self.proc,
            analysis,
            buffer,
            target_id,
        )?;
        let runtime_offsets = if let Some(ref o) = self.runtime_offsets {
            o.clone()
        } else {
            self.runtime_offsets = Some(plan.runtime_offsets);
            self.runtime_offsets.clone().unwrap()
        };
        let not_instrumented = trampoline::NotInstrumented {
            tracee: ptrace::Attached::try_from(self.proc)?,
            targets: plan.targets,
            runtime_offsets,
        };
        Ok(Self {
            proc: not_instrumented.instrument()?,
            ..self
        })
    }
}
