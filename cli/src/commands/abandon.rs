// Copyright 2020 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Write;

use jj_lib::backend::ObjectId;
use tracing::instrument;

use crate::cli_util::{
    resolve_multiple_nonempty_revsets, CommandError, CommandHelper, RevisionArg,
};
use crate::ui::Ui;

/// Abandon a revision
///
/// Abandon a revision, rebasing descendants onto its parent(s). The behavior is
/// similar to `jj restore --changes-in`; the difference is that `jj abandon`
/// gives you a new change, while `jj restore` updates the existing change.
#[derive(clap::Args, Clone, Debug)]
pub(crate) struct AbandonArgs {
    /// The revision(s) to abandon
    #[arg(default_value = "@")]
    revisions: Vec<RevisionArg>,
    /// Do not print every abandoned commit on a separate line
    #[arg(long, short)]
    summary: bool,
    /// Ignored (but lets you pass `-r` for consistency with other commands)
    #[arg(short = 'r', hide = true)]
    unused_revision: bool,
}

#[instrument(skip_all)]
pub(crate) fn cmd_abandon(
    ui: &mut Ui,
    command: &CommandHelper,
    args: &AbandonArgs,
) -> Result<(), CommandError> {
    let mut workspace_command = command.workspace_helper(ui)?;
    let to_abandon = resolve_multiple_nonempty_revsets(&args.revisions, &workspace_command, ui)?;
    workspace_command.check_rewritable(to_abandon.iter())?;
    let transaction_description = if to_abandon.len() == 1 {
        format!("abandon commit {}", to_abandon[0].id().hex())
    } else {
        format!(
            "abandon commit {} and {} more",
            to_abandon[0].id().hex(),
            to_abandon.len() - 1
        )
    };
    let mut tx = workspace_command.start_transaction(&transaction_description);
    for commit in &to_abandon {
        tx.mut_repo().record_abandoned_commit(commit.id().clone());
    }
    let num_rebased = tx.mut_repo().rebase_descendants(command.settings())?;

    if to_abandon.len() == 1 {
        write!(ui.stderr(), "Abandoned commit ")?;
        tx.base_workspace_helper()
            .write_commit_summary(ui.stderr_formatter().as_mut(), &to_abandon[0])?;
        writeln!(ui.stderr())?;
    } else if !args.summary {
        writeln!(ui.stderr(), "Abandoned the following commits:")?;
        for commit in to_abandon {
            write!(ui.stderr(), "  ")?;
            tx.base_workspace_helper()
                .write_commit_summary(ui.stderr_formatter().as_mut(), &commit)?;
            writeln!(ui.stderr())?;
        }
    } else {
        writeln!(ui.stderr(), "Abandoned {} commits.", &to_abandon.len())?;
    }
    if num_rebased > 0 {
        writeln!(
            ui.stderr(),
            "Rebased {num_rebased} descendant commits onto parents of abandoned commits"
        )?;
    }
    tx.finish(ui)?;
    Ok(())
}
