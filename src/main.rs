mod app;

fn main() -> Result<(), app::AppError> {
    let mut app = app::App::new()?;
    'main: loop {
        if app.process_input(50)? == app::InputState::Quit {
            break 'main;
        }
        app.update()?;
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    app.exit()?;
    Ok(())
}
