use anyhow::Result;
use vergen_gitcl::{Emitter, Gitcl};

fn main() -> Result<()> {
    Emitter::default()
        .add_instructions(&Gitcl::all().sha(true).build())?
        .emit()?;
    Ok(())
}
