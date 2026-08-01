//! `vmate export`: export successful configs.

use crate::cli::ExportArgs;
use crate::settings::Settings;
use anyhow::Result;
use vmate_core::db::ConfigRepo;
use vmate_core::db::pool::init_pool;
use vmate_core::export::export_configs;
use vmate_core::paths;

pub async fn run(settings: &Settings, args: &ExportArgs) -> Result<()> {
    let pool = init_pool(&settings.db_path).await?;
    let repo = ConfigRepo::new(pool);
    let dest = paths::expand_path(&args.out);

    let result = export_configs(&repo, &settings.filter, &dest).await?;
    println!(
        "Exported {} of {} configs to {}",
        result.exported,
        result.total,
        result.dest.display()
    );
    Ok(())
}
