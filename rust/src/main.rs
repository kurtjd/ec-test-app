use clap::Parser;
use color_eyre::Result;
use ec_demo::app::{App, AppArgs};

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    #[cfg(not(feature = "mock"))]
    let source = ec_demo::acpi::Acpi::new();

    #[cfg(feature = "mock")]
    let source = ec_demo::mock::Mock::default();

    let args = AppArgs::parse();
    App::new(source, args).run(terminal)
}
