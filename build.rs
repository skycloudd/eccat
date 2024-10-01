use vergen::{BuildBuilder, Emitter, RustcBuilder, SysinfoBuilder};
use vergen_git2::Git2Builder;

fn main() -> anyhow::Result<()> {
    // vergen::EmitBuilder::builder()
    //     .build_date()
    //     .git_branch()
    //     .git_describe(true, true, None)
    //     .rustc_semver()
    //     .sysinfo_name()
    //     .emit()?;

    let build = BuildBuilder::default().build_date(true).build()?;
    let git = Git2Builder::default()
        .branch(true)
        .describe(true, true, None)
        .build()?;
    let rustc = RustcBuilder::default().semver(true).build()?;
    let si = SysinfoBuilder::default().name(true).build()?;

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&git)?
        .add_instructions(&rustc)?
        .add_instructions(&si)?
        .emit()?;

    Ok(())
}
