use anyhow::Result;

fn main() -> Result<()> {
    #[cfg(feature = "cli")]
    {
        use vergen_gitcl::{Emitter, Gitcl};
        Emitter::default()
            .add_instructions(&Gitcl::all().sha(true).build())?
            .emit()?;
    }
    Ok(())
}
