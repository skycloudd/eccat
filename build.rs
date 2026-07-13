use vergen::{Build, Emitter, Rustc, Sysinfo};
use vergen_git2::Git2;

fn main() -> anyhow::Result<()> {
    let build = Build::builder().build_date(true).build();
    let git = Git2::builder()
        .branch(true)
        .describe(true, true, None)
        .build();
    let rustc = Rustc::builder().semver(true).build();
    let si = Sysinfo::builder().name(true).build();

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&git)?
        .add_instructions(&rustc)?
        .add_instructions(&si)?
        .emit()?;

    Ok(())
}
