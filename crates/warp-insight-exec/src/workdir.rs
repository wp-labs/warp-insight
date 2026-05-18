//! Workdir protocol helpers.

use std::io;
use std::path::{Path, PathBuf};

use warp_insight_contracts::action_plan::ActionPlanContract;
use warp_insight_contracts::action_result::ActionResultContract;
pub use warp_insight_contracts::state_exec::{ExecProgressState, ExecRuntimeContext};
use warp_insight_shared::fs::{read_json, write_json_atomic};
use warp_insight_shared::paths::{
    WORKDIR_PLAN_FILE, WORKDIR_RESULT_FILE, WORKDIR_RUNTIME_FILE, WORKDIR_STATE_FILE,
};

#[derive(Debug, Clone)]
pub struct ExecutionWorkdir {
    pub root: PathBuf,
    pub plan_path: PathBuf,
    pub runtime_path: PathBuf,
    pub state_path: PathBuf,
    pub result_path: PathBuf,
}

impl ExecutionWorkdir {
    pub fn open(base: &Path) -> io::Result<Self> {
        if !base.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("workdir does not exist: {}", base.display()),
            ));
        }

        Ok(Self {
            root: base.to_path_buf(),
            plan_path: base.join(WORKDIR_PLAN_FILE),
            runtime_path: base.join(WORKDIR_RUNTIME_FILE),
            state_path: base.join(WORKDIR_STATE_FILE),
            result_path: base.join(WORKDIR_RESULT_FILE),
        })
    }

    pub fn read_plan(&self) -> io::Result<ActionPlanContract> {
        read_json(&self.plan_path)
    }

    pub fn read_runtime(&self) -> io::Result<ExecRuntimeContext> {
        read_json(&self.runtime_path)
    }

    pub fn write_state(&self, state: &ExecProgressState) -> io::Result<()> {
        write_json_atomic(&self.state_path, state)
    }

    pub fn write_result(&self, result: &ActionResultContract) -> io::Result<()> {
        write_json_atomic(&self.result_path, result)
    }
}
