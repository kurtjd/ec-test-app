use color_eyre::Result;
use ec_demo::app::App;
use ec_demo::notifications::Notifications;

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    #[cfg(not(feature = "mock"))]
    let source = ec_demo::acpi::Acpi::default();

    #[cfg(feature = "mock")]
    let source = ec_demo::mock::Mock::default();

    let notifications = Notifications::new()?;
    App::new(source, &notifications).run(terminal)
}
