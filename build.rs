fn main() -> anyhow::Result<()> {
    vergen::EmitBuilder::builder()
        .build_date()
        .git_branch()
        .git_describe(true, true, None)
        .rustc_semver()
        .sysinfo_name()
        .emit()?;

    Ok(())
}
