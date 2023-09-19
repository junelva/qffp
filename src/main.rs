mod app;

fn main() -> Result<(), app::AppError> {
    let mut app = app::App::new()?;
    'main: loop {
        if app.process_input(50)? == app::InputState::Quit {
            break 'main;
        }
        app.update()?;
    }

    app.exit()?;
    Ok(())
}
