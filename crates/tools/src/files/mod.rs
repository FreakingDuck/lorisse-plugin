//! Files: `attach` keeps a copy of a file of the person's machine with the memory or the person
//! it is about, as one commit; the original stays where it was.

use std::path::Path;

use lorisse_store::{Layer, StoreError};

use crate::reading::folder;
use crate::shared::write::{piece_of, refused, short};
use crate::shared::{Answer, Memory};

pub(crate) fn attach(memory: &mut Memory, root: &Path, piece: &str, path: &str) -> Answer {
    let what = "a file goes with a memory or a person";
    let id = match piece_of(memory, piece, &[Layer::Memory, Layer::Person], what) {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    match memory.store.attach(&id, Path::new(path)) {
        Ok(attached) => {
            let label = memory.label_of(&attached.id, &attached.repo);
            let kept = folder::shown(&root.join(&attached.path));
            Answer::ok(match &attached.commit {
                Some(commit) => format!(
                    "Attached {} to {label}: the copy is kept at {kept}, in one commit of {} ({}); the original stays where it was, and undo takes it back.",
                    attached.name,
                    attached.repo,
                    short(commit)
                ),
                None => format!(
                    "Attached {} to {label}: the copy is kept at {kept}; the original stays where it was.",
                    attached.name
                ),
            })
        }
        Err(StoreError::Invalid(reason)) | Err(StoreError::Git(reason)) => Answer::error(format!(
            "Nothing was copied: {}.",
            reason.trim_end_matches('.')
        )),
        Err(e) => refused(e),
    }
}
